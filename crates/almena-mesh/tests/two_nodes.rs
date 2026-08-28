//! Two nodes, two sockets, and one catching up with the other.
//!
//! **The check the whole mesh is for.** Everything under it is testable without a network; this is
//! not, and it is what would catch a wire format that reads back differently, a protocol name that
//! does not match, or a record that arrives and is not accepted.
//!
//! What it does not check, and must not appear to: that acts are believed because they came from a
//! node. They are not. What arrives goes through the same admission as an act handed over by a
//! stranger, because that is what it is — so the assertion at the end is about the **record**, and
//! never about what was said over the wire.

use almena_mesh::sync::{Ask, Said};
use almena_mesh::{Happened, Listening};
use almena_node::{Answer, Epoch, Node, Opening, SigningKey, Which};

/// How long anything here is given before it is called a failure.
const PATIENCE: std::time::Duration = std::time::Duration::from_secs(20);

/// A key nobody else in this test has.
fn key(seed: u8) -> SigningKey {
    SigningKey::from_secret([seed; 32])
}

/// A development network with a fixed clock, so a test is never about what time it is.
fn at() -> Opening {
    Opening {
        which: Which::Development,
        beginning: Epoch::GENESIS,
        began: 1_800_000_000,
    }
}

/// A holder account, signed by the key that will control it.
fn an_account(control: &SigningKey) -> almena_format::operation::Operation {
    use almena_format::cbor::Value;
    use almena_format::identifier::Network;
    use almena_format::operation::{Signed, create};
    use std::collections::BTreeMap;

    let public = control.verifying_key().bytes();
    let mut operation = create(
        Network::Development,
        almena_store::kind::Kind::HOLDER_CREATE.number(),
        1,
        Epoch::GENESIS,
        BTreeMap::from([(1, Value::Bytes(public.to_vec()))]),
    );
    let signature = control.sign(&operation.signing_bytes());
    operation.signatures.push(Signed {
        by: operation.object.clone(),
        key: public.to_vec(),
        signature: signature.bytes(),
    });
    operation
}

/// Run a node until it says where it can be reached, on the address a test may dial.
async fn reachable(listening: &mut Listening) -> libp2p::Multiaddr {
    loop {
        if let Happened::Reachable(address) = listening.next().await {
            // Loopback, because a test that dialled this machine's public address would be a test
            // about somebody's router.
            if address.to_string().starts_with("/ip4/127.0.0.1") {
                return address;
            }
        }
    }
}

/// Drive both nodes until the one that asked has an answer.
///
/// They are driven **together**, because each has to keep answering while the other is asking. That
/// is the whole difference between a mesh and a client calling a server.
async fn exchange(serving: &mut Listening, asking: &mut Listening, ahead: &Node) -> Said {
    let mut asked = false;
    loop {
        tokio::select! {
            happened = serving.next() => {
                if let Happened::Asked(_, Ask::Since(from), back) = happened {
                    // Handed on in the bytes this node holds them in. Nothing is re-encoded: the
                    // name of an act is the hash of its bytes.
                    let acts = ahead.since(
                        from,
                        almena_node::Page {
                            at_most: 64,
                            weighing_at_most: 4 * 1024 * 1024,
                        },
                        Epoch::GENESIS,
                    ).answer;
                    let written = ahead.written() as u64;
                    let _ = serving.answer(
                            back,
                            Said {
                                acts,
                                written,
                                // Nothing was asked about an epoch, so nothing is said about one.
                                root: None,
                            },
                        );
                }
            }
            happened = asking.next() => match happened {
                Happened::Met(peer) if !asked => {
                    asked = true;
                    asking.ask(&peer, Ask::Since(0));
                }
                Happened::Answered(_, said) => return said,
                _ => {}
            },
        }
    }
}

/// Put everything that arrived through the node's own admission, and say how much was kept.
///
/// **The point of the whole test.** What decides whether an act is kept is the act's own signature,
/// not the peer that sent it — so this is the same call a stranger's act goes through, made from
/// bytes that happened to come off a wire.
fn admit_all(node: &mut Node, said: &Said) -> usize {
    let mut taken = 0;
    for act in &said.acts {
        let Ok(value) = almena_format::cbor::read(act) else {
            panic!("what came over the wire is not canonical bytes")
        };
        let Some(operation) = almena_format::operation::read(&value) else {
            panic!("what came over the wire is not an act")
        };
        if node.submit(&operation, Epoch::GENESIS).is_ok() {
            taken += 1;
        }
    }
    taken
}

/// The record a zone would publish for a node listening there.
///
/// Written exactly as it goes in a zone file and read back by the same code that reads a real one,
/// so that a test cannot pass on a record nobody could publish.
fn published_for(address: &libp2p::Multiaddr, network: &str) -> almena_node::zone::Seed {
    let Some(port) = address
        .to_string()
        .split("/tcp/")
        .nth(1)
        .and_then(|rest| rest.split('/').next())
        .map(str::to_owned)
    else {
        panic!("an address with a port in it")
    };

    let written = format!(
        "v=1 host=localhost port={port} peer={} net={network}",
        almena_node::peer::of(&key(6).verifying_key())
    );
    let Ok(seed) = almena_node::zone::Seed::read(&written) else {
        panic!("a record a zone could publish")
    };
    seed
}

#[tokio::test(flavor = "multi_thread")]
async fn a_node_that_knows_nothing_joins_by_what_the_zone_published() {
    // **The whole loop, end to end.** A zone says who to call; a node calls them, is handed the
    // record, and becomes a node on that network — having opened nothing and trusted nobody.
    let government = key(5);
    let mut first = Node::open(&at(), &[], &government, key(6)).expect("nobody to join");
    let account = an_account(&key(9));
    let named = account.object.name().clone();
    first.submit(&account, Epoch::GENESIS).expect("taken");

    let network = first.network().as_str().to_owned();
    let mut serving = almena_mesh::listen(&key(6), &network, 0).expect("a place");
    let mut arriving = almena_mesh::listen(&key(8), &network, 0).expect("a place");

    let address = tokio::time::timeout(PATIENCE, reachable(&mut serving))
        .await
        .expect("the first node should be listening");
    tokio::time::timeout(PATIENCE, reachable(&mut arriving))
        .await
        .expect("the arriving node should be listening");

    let seed = published_for(&address, &network);
    let dialling = almena_mesh::dialling(&seed).expect("somewhere to dial");

    arriving.dial(dialling).expect("dialled");
    let handed = tokio::time::timeout(PATIENCE, exchange(&mut serving, &mut arriving, &first))
        .await
        .expect("the record should arrive");

    let scratch = std::env::temp_dir().join("almena-mesh-joins");
    let _ = std::fs::remove_dir_all(&scratch);
    let joined = Node::join(
        &scratch,
        key(8),
        almena_node::Joining {
            acts: &handed.acts,
            network: seed.network(),
        },
        Epoch::GENESIS,
    )
    .expect("joined");

    assert_eq!(
        joined.network().as_str(),
        seed.network(),
        "and what arrived is the network the zone promised, which is the point of it saying so"
    );
    assert_eq!(joined.network(), first.network(), "the same network");
    assert_ne!(joined.did(), first.did(), "and a node of its own on it");
    assert!(
        matches!(
            joined.resolve(&named, Epoch::GENESIS).answer,
            Answer::Here(_)
        ),
        "holding what the zone's node had been told"
    );
    let _ = std::fs::remove_dir_all(&scratch);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_node_catches_up_with_one_that_has_more() {
    // One network and two nodes on it. They are two because they hold two keys — which is what
    // being two nodes is — and the second knows nothing the first was told.
    let government = key(5);
    let mut ahead = Node::open(&at(), &[], &government, key(6)).expect("nobody to join");
    let mut behind = Node::open(&at(), &[], &government, key(7)).expect("nobody to join");
    assert_eq!(ahead.network(), behind.network(), "one network");

    let account = an_account(&key(9));
    let named = account.object.name().clone();
    ahead.submit(&account, Epoch::GENESIS).expect("taken");
    assert!(
        matches!(
            behind.resolve(&named, Epoch::GENESIS).answer,
            Answer::DoesNotExist
        ),
        "and the other one has never heard of it"
    );

    let network = ahead.network().as_str().to_owned();
    let mut serving = almena_mesh::listen(&key(6), &network, 0).expect("a place");
    let mut asking = almena_mesh::listen(&key(7), &network, 0).expect("a place");

    let address = tokio::time::timeout(PATIENCE, reachable(&mut serving))
        .await
        .expect("the first node should be listening");
    tokio::time::timeout(PATIENCE, reachable(&mut asking))
        .await
        .expect("the second node should be listening");

    asking.dial(address).expect("dialled");

    let caught_up = tokio::time::timeout(PATIENCE, exchange(&mut serving, &mut asking, &ahead))
        .await
        .expect("an answer should arrive");

    assert_eq!(
        caught_up.written,
        ahead.written() as u64,
        "and it says how far it has got, so a short answer is not mistaken for the end"
    );
    assert!(!caught_up.acts.is_empty());

    let taken = admit_all(&mut behind, &caught_up);
    assert!(taken > 0, "the acts it did not already hold were taken");

    assert!(
        matches!(
            behind.resolve(&named, Epoch::GENESIS).answer,
            Answer::Here(_)
        ),
        "the account the other node was told about is one this node now holds"
    );
}
