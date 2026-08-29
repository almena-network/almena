//! A node that cannot be dialled, reached through one that agreed to carry it.
//!
//! **The case that decides whether *open and without permission* is true for anybody but people
//! with a public address.** Behind a household router there is no door anybody outside can knock
//! on, and a machine like that could hold the whole record and still answer nothing. Somebody
//! carrying it is what turns it back into a node.
//!
//! It is testable without a router: a node that asks to be carried gets its way in through somebody
//! else's socket whether or not its own would have worked, so what is checked here is the thing
//! that matters — a slot asked for, a slot granted, and an address that runs through the relay.
//!
//! What it does not check, and must not appear to: that being carried makes anything believed. What
//! arrives over a circuit goes through the same admission as anything else, because a relay carries
//! bytes and vouches for none of them.

use almena_mesh::{Carrying, Happened, Listening};
use almena_node::{Epoch, SigningKey};

/// How long anything here is given before it is called a failure.
const PATIENCE: std::time::Duration = std::time::Duration::from_secs(20);

/// A key nobody else in this test has.
fn key(seed: u8) -> SigningKey {
    SigningKey::from_secret([seed; 32])
}

/// Run a node until it says where it can be reached, on the address a test may dial.
async fn reachable(listening: &mut Listening) -> libp2p::Multiaddr {
    loop {
        if let Happened::Reachable(address) = listening.next().await {
            return address;
        }
    }
}

/// The name a key answers to on the mesh, for putting at the end of an address.
fn named(seed: u8) -> libp2p::PeerId {
    let Ok(identity) = almena_mesh::identity(&key(seed)) else {
        panic!("a key in use makes an identity")
    };
    identity.public().to_peer_id()
}

/// A relay listening and reachable, with the address somebody would ask it at.
async fn a_relay(seed: u8, network: &str) -> (Listening, libp2p::Multiaddr) {
    let Ok(mut relay) = almena_mesh::listening(&key(seed), network, 0, Carrying::ForOthers) else {
        panic!("a relay should get a place on the mesh")
    };
    let Ok(at) = tokio::time::timeout(PATIENCE, reachable(&mut relay)).await else {
        panic!("the relay should be listening well within that")
    };
    let through = at.with(libp2p::multiaddr::Protocol::P2p(named(seed)));
    (relay, through)
}

#[tokio::test]
async fn a_node_that_cannot_be_dialled_is_reachable_through_one_that_carries_it() {
    let network = "carried";

    // One node volunteers to carry traffic. Nothing about it is different otherwise, which is the
    // point: carrying is something a node does, not something a node is.
    let (mut relay, through) = a_relay(1, network).await;
    assert!(relay.carries(), "it said it would carry traffic");

    // And one that does not carry anybody asks to be carried.
    let mut behind = almena_mesh::listening(&key(2), network, 0, Carrying::ForNobody)
        .expect("a node should get a place on the mesh");
    assert!(!behind.carries(), "it did not volunteer, so it does not");
    behind
        .ask_to_be_carried(&through)
        .expect("asking a named relay is a thing that can be asked");

    // Both have to be run for the reservation to be made at all: one side asks over a connection
    // and the other side answers, and a test that only drove one would be waiting for nobody.
    let granted = tokio::time::timeout(PATIENCE, async {
        loop {
            tokio::select! {
                _ = relay.next() => {}
                happened = behind.next() => {
                    if let Happened::Carried(address) = happened {
                        return address;
                    }
                }
            }
        }
    })
    .await;
    let Ok(address) = granted else {
        panic!("the relay should have granted a slot well within that")
    };

    assert!(
        almena_mesh::borrowed(&address),
        "it is reachable there because somebody is carrying it, and that is not the same fact"
    );
    // The way in runs through the relay, at an address the relay said it had — and not at the one
    // this node happened to dial it on. A relay lends what it can be reached at, which is not the
    // same as where somebody found it.
    assert!(
        address
            .iter()
            .any(|part| part == libp2p::multiaddr::Protocol::P2p(named(1))),
        "the way in names the relay that granted it"
    );
    assert!(
        almena_mesh::worth_publishing(&address),
        "and it is somewhere somebody else could use, or it is not a way in"
    );
}

#[tokio::test]
async fn losing_one_relay_is_losing_one_relay() {
    // A node carried by two that dropped every circuit when either ended would withdraw an address
    // that still answers — wrong in the honest-looking direction, which is still wrong.
    let network = "carried-twice";
    let (mut first, through_first) = a_relay(11, network).await;
    let (mut second, through_second) = a_relay(12, network).await;

    let mut behind = almena_mesh::listening(&key(13), network, 0, Carrying::ForNobody)
        .expect("a node should get a place on the mesh");
    behind
        .ask_to_be_carried(&through_first)
        .expect("a named relay can be asked");
    behind
        .ask_to_be_carried(&through_second)
        .expect("and so can a second one");

    both_granted(&mut first, &mut second, &mut behind).await;

    // The first relay goes away, taking its slot with it. What must survive is the second's.
    drop(first);
    let lost = one_stopped(&mut second, &mut behind).await;
    assert_eq!(lost, named(11), "and it names the relay that stopped");

    let still: Vec<String> = behind
        .addresses()
        .iter()
        .filter(|address| almena_mesh::borrowed(address))
        .map(ToString::to_string)
        .collect();
    assert!(
        still
            .iter()
            .any(|address| address.contains(&named(12).to_string())),
        "the second relay's circuit still answers and is still held"
    );
    assert!(
        !still
            .iter()
            .any(|address| address.contains(&named(11).to_string())),
        "and the first relay's is gone"
    );
}

/// Drive everybody until both slots are granted.
async fn both_granted(first: &mut Listening, second: &mut Listening, behind: &mut Listening) {
    let granted = tokio::time::timeout(PATIENCE, async {
        let mut have = 0;
        loop {
            tokio::select! {
                _ = first.next() => {}
                _ = second.next() => {}
                happened = behind.next() => {
                    if matches!(happened, Happened::Carried(_)) {
                        have += 1;
                        if have == 2 {
                            return;
                        }
                    }
                }
            }
        }
    })
    .await;
    assert!(
        granted.is_ok(),
        "both relays should have granted a slot well within that"
    );
}

/// Drive what is left until a slot is reported gone, and say whose it was.
async fn one_stopped(second: &mut Listening, behind: &mut Listening) -> libp2p::PeerId {
    let told = tokio::time::timeout(PATIENCE, async {
        loop {
            tokio::select! {
                _ = second.next() => {}
                happened = behind.next() => {
                    if let Happened::NotCarried(relay) = happened {
                        return relay;
                    }
                }
            }
        }
    })
    .await;
    let Ok(lost) = told else {
        panic!("the slot ending should have been said well within that")
    };
    lost
}

#[tokio::test]
async fn a_relay_has_to_be_somebody_and_not_a_host_and_a_port() {
    // A circuit runs through a node. Being carried by whoever happens to answer at a host and port
    // is being carried by whoever took that host and port, which is the one thing a name prevents.
    let mut asking = almena_mesh::listen(&key(3), "carried", 0).expect("a place on the mesh");
    let nameless = "/ip4/198.51.100.7/tcp/4001".parse().expect("an address");
    assert_eq!(
        asking.ask_to_be_carried(&nameless),
        Err(almena_mesh::NotListening::Anonymous)
    );
    assert_eq!(
        asking.ask_to_be_carried_at("not an address at all"),
        Err(almena_mesh::NotListening::AddressUnavailable)
    );
}

#[test]
fn an_address_only_this_machine_could_use_is_not_a_way_in() {
    // Loopback is where **every** machine is. A node that published it would be publishing an
    // address that, on the reader's machine, resolves to the reader — and where the nodes are is a
    // figure anybody can read, so one address every node shares would make a network on one desk
    // look as spread out as one across three countries.
    for nowhere in [
        "/ip4/127.0.0.1/tcp/4001",
        "/ip6/::1/tcp/4001",
        "/ip4/0.0.0.0/tcp/4001",
        "/ip4/169.254.3.9/tcp/4001",
    ] {
        let address: libp2p::Multiaddr = nowhere.parse().expect("an address");
        assert!(
            !almena_mesh::worth_publishing(&address),
            "{nowhere} is not somewhere anybody else could reach this node"
        );
    }
    let real: libp2p::Multiaddr = "/ip4/198.51.100.7/tcp/4001".parse().expect("an address");
    assert!(almena_mesh::worth_publishing(&real));
}

#[test]
fn what_the_record_says_about_where_a_node_is_only_ever_comes_from_the_node() {
    // A circuit is published because nobody could have known it: it does not exist until a relay
    // agrees. An address of the node's own is chosen — it is the one somebody puts in a zone — and
    // is not written down here, because deciding it would be deciding on the operator's behalf.
    let mut node = almena_node::Node::open(
        &almena_node::Opening {
            which: almena_node::Which::Development,
            beginning: Epoch::GENESIS,
            began: 1_800_000_000,
        },
        &[],
        &key(4),
        key(5),
    )
    .expect("a node to be opened");

    let mine = node.did().name().clone();
    assert!(
        node.reachable_at(&mine).is_empty(),
        "a node that has said nothing about where it is has said nothing"
    );

    let circuit = std::collections::BTreeSet::from([
        "/ip4/198.51.100.7/tcp/4001/p2p/12D3KooWA/p2p-circuit".to_owned(),
    ]);
    assert!(node.also_reachable_at(&circuit, Epoch::GENESIS));
    assert_eq!(node.reachable_at(&mine), circuit);

    // Saying it twice is not two acts. What the record already says needs nothing said about it.
    assert!(
        !node.also_reachable_at(&circuit, Epoch::GENESIS),
        "there is nothing to add"
    );

    // And a relay that stops carrying it leaves an address that answers nothing, which is worse
    // than saying nothing: whoever reads it goes and knocks.
    assert!(node.no_longer_reachable_at(&circuit, Epoch::GENESIS));
    assert!(node.reachable_at(&mine).is_empty());
    assert!(
        !node.no_longer_reachable_at(&circuit, Epoch::GENESIS),
        "there is nothing left to take away"
    );
}
