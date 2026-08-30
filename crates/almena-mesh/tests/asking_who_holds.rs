//! Two nodes, and one of them going and finding out whether the other still has what it was dealt.
//!
//! **This is the difference between a rule and a measurement.** Anybody can work out which nodes a
//! thing was dealt to; whether they still have it is only answered by asking them to hand it over
//! and being handed something that hashes to what was asked for. A claim would be worth nothing —
//! and this is the one question on the mesh whose answer nobody has to be believed about, because
//! the hash it is checked against is in the log and the log is everybody's.
//!
//! Nothing in this file asks anybody anything. Both nodes are handed to the thing that keeps them
//! up to date and left alone, and what is asserted is that the one which went looking wrote down
//! what it found.

use std::sync::Arc;

use almena_node::{Epoch, Node, Opening, SigningKey, Which};
use tokio::sync::RwLock;

/// How long they are given before it is called a failure.
const PATIENCE: std::time::Duration = std::time::Duration::from_secs(30);

/// How often a node asks whoever it knows for what came next.
///
/// Short here, unlike the tests about telling: what is under test is the looking, which happens on
/// the same slow tick as the rest of a node's daily work, and it has to come round inside the wait.
const OFTEN: std::time::Duration = std::time::Duration::from_millis(200);

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

/// Run a node until it says where it can be reached, on an address this test may dial.
async fn reachable(listening: &mut almena_mesh::Listening) -> libp2p::Multiaddr {
    loop {
        if let almena_mesh::Happened::Reachable(address) = listening.next().await
            && address.to_string().starts_with("/ip4/127.0.0.1")
        {
            return address;
        }
    }
}

/// What an act off the wire is called.
///
/// **By what it says and not by how it was signed**, which is the only reading under which a peer
/// handing over the same act in the other of a signature's two valid forms has handed over the
/// thing that was asked for — which it has.
fn named(bytes: &[u8]) -> Option<almena_format::identifier::Name> {
    let value = almena_format::cbor::read(bytes).ok()?;
    Some(almena_format::operation::read(&value)?.called())
}

/// Wait for something to become true, and say whether it did.
async fn until<F, Fut>(yet: F) -> bool
where
    F: Fn() -> Fut,
    Fut: Future<Output = bool>,
{
    tokio::time::timeout(PATIENCE, async {
        while !yet().await {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    })
    .await
    .is_ok()
}

/// Two nodes on one network, one dialling the other, and both left to it.
async fn two_left_to_it() -> Vec<Arc<RwLock<Node>>> {
    let government = key(5);
    let mut nodes = Vec::new();
    let mut places = Vec::new();

    for seed in [6u8, 7] {
        let Ok(node) = Node::open(&at(), &[], &government, key(seed)) else {
            panic!("there is nobody to join")
        };
        let network = node.network().as_str().to_owned();
        let Ok(mut place) = almena_mesh::listen(&key(seed), &network, 0) else {
            panic!("every node should get a place on the mesh")
        };
        let Ok(address) = tokio::time::timeout(PATIENCE, reachable(&mut place)).await else {
            panic!("every node should be listening well within that")
        };
        nodes.push(Arc::new(RwLock::new(node)));
        places.push((place, address));
    }

    // **One dials, the other is dialled**, which is enough for both: a connection is a connection
    // whichever end opened it, so both learn of each other and either may go looking. Having them
    // dial each other at once does not work today — see what is recorded about it — and it is not
    // what is under test here.
    let mut behind: Vec<libp2p::Multiaddr> = Vec::new();
    for (which, (place, address)) in places.into_iter().enumerate() {
        tokio::spawn(almena_mesh::keeping::keeping_up(
            place,
            Arc::clone(&nodes[which]),
            behind,
            || Epoch::GENESIS,
            OFTEN,
        ));
        behind = vec![address];
    }
    nodes
}

/// Something for them to be asked about, told to one of them.
async fn told_some_things(node: &Arc<RwLock<Node>>) {
    for which in 0..6 {
        let account = an_account(&key(20 + which));
        assert!(
            node.write().await.submit(&account, Epoch::GENESIS).is_ok(),
            "taken"
        );
    }
}

/// Wait until both hold the same record and the record names both of them.
///
/// **Both have to be true before the share-out deals either of them anything**: a node the record
/// has never named is not in the census, and a thing this node has not got is not one it can go
/// asking after.
async fn settled(nodes: &[Arc<RwLock<Node>>]) {
    assert!(
        until(|| async {
            let one = nodes[0].read().await.written();
            let other = nodes[1].read().await.written();
            one == other && one >= 8
        })
        .await,
        "both should end up holding the same record"
    );
    assert!(
        until(|| async { nodes[0].read().await.share_out(Epoch::GENESIS).1.len() == 2 }).await,
        "and the record should name both of them"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_node_goes_and_finds_out_whether_the_others_still_have_what_they_were_dealt() {
    let nodes = two_left_to_it().await;

    told_some_things(&nodes[0]).await;
    settled(&nodes).await;

    // Now the thing under test: one of them asks the other to hand something over, and what comes
    // back is checked against the hash the log already carries.
    assert!(
        until(|| async {
            let node = nodes[0].read().await;
            let thing = node.at_sequence(0).expect("a record with something in it");
            let holders = node
                .holders_of(&thing, almena_node::COPIES_OF_HISTORY, Epoch::GENESIS)
                .answer;
            // With two nodes and five copies asked for, a thing falls to both of them.
            holders.len() == 2
        })
        .await,
        "a thing falls to as many nodes as there are when there are few"
    );

    // And what it went looking for it found: the peer really can hand over what it was dealt, and
    // what comes back hashes to what was asked for.
    assert!(
        until(|| async {
            let thing = nodes[0]
                .read()
                .await
                .at_sequence(0)
                .expect("a record with something in it");
            nodes[1]
                .read()
                .await
                .act(&thing, Epoch::GENESIS)
                .answer
                .is_some_and(|bytes| named(&bytes) == Some(thing))
        })
        .await,
        "the peer a thing was dealt to should be able to hand over exactly it"
    );
}
