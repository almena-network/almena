//! Nodes finding each other when nobody arranged who calls whom.
//!
//! The other tests here have one node dial and the other wait, which is the shape of a node joining
//! a network that is already there. This is the other shape: two seeds coming up together and each
//! dialling the other in the same instant, a node that went away and came back, and a node that
//! holds the record and has nobody's zone to ask. **All three are what a network looks like at
//! the moment it forms or re-forms**, which is the moment it must not fail to.

use std::sync::Arc;

use almena_mesh::{Happened, Listening};
use almena_node::{Epoch, Node, Opening, SigningKey, Which};
use libp2p::multiaddr::Protocol;
use tokio::sync::RwLock;

/// How long anything here is given before it is called a failure.
const PATIENCE: std::time::Duration = std::time::Duration::from_secs(20);

/// How often a node asks whoever it knows. Longer than the test, so that asking is not what is
/// being measured.
const OFTEN: std::time::Duration = std::time::Duration::from_secs(120);

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

/// The name a key answers to on the mesh.
fn named(seed: u8) -> libp2p::PeerId {
    let Ok(identity) = almena_mesh::identity(&key(seed)) else {
        panic!("a key in use makes an identity")
    };
    identity.public().to_peer_id()
}

/// Run a node until it says where it can be reached, on the address a test may dial.
async fn reachable(listening: &mut Listening) -> libp2p::Multiaddr {
    loop {
        if let Happened::Reachable(address) = listening.next().await
            && address.to_string().starts_with("/ip4/127.0.0.1")
        {
            return address;
        }
    }
}

/// A node listening somewhere, and the address somebody would dial it at — identity and all.
async fn a_place(seed: u8, network: &str, port: u16) -> (Listening, libp2p::Multiaddr) {
    let Ok(mut listening) = almena_mesh::listen(&key(seed), network, port) else {
        panic!("a node should get a place on the mesh")
    };
    let Ok(address) = tokio::time::timeout(PATIENCE, reachable(&mut listening)).await else {
        panic!("it should be listening well within that")
    };
    (listening, address.with(Protocol::P2p(named(seed))))
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

/// A node driven and nothing more, so that it can be taken away by dropping what drives it.
fn driven(listening: Listening) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut listening = listening;
        loop {
            let _ = listening.next().await;
        }
    })
}

/// One node's record, handed to another act by act through the same admission as anything else.
fn handed_over(from: &Node, to: &mut Node) {
    let page = almena_node::Page {
        at_most: 256,
        weighing_at_most: 4 * 1024 * 1024,
    };
    for bytes in from.since(0, page, Epoch::GENESIS).answer {
        let Ok(value) = almena_format::cbor::read(&bytes) else {
            panic!("a record is canonical bytes")
        };
        let Some(act) = almena_format::operation::read(&value) else {
            panic!("a record is acts")
        };
        // The act that opened the network is already held; the rest are the other node's own.
        let _ = to.submit(&act, Epoch::GENESIS);
    }
}

/// Drive both until each has met the other, or the time is up.
async fn both_met(one: &mut Listening, other: &mut Listening) -> (bool, bool) {
    let (mut one_met, mut other_met) = (false, false);
    let _ = tokio::time::timeout(PATIENCE, async {
        while !(one_met && other_met) {
            tokio::select! {
                happened = one.next() => {
                    if let Happened::Met(who, _) = happened && who == named(7) { one_met = true; }
                }
                happened = other.next() => {
                    if let Happened::Met(who, _) = happened && who == named(6) { other_met = true; }
                }
            }
        }
    })
    .await;
    (one_met, other_met)
}

#[tokio::test(flavor = "multi_thread")]
async fn two_nodes_that_dial_each_other_in_the_same_instant_both_get_through() {
    // **Two seeds coming up together.** Each is told about the other and each dials, in the same
    // tick. Dialled from the listening port this is one connection opened from both ends at once
    // and a handshake in which both sides try to speak first — nobody gets through, and the
    // network never forms. Dialled from a fresh port each it is two connections, and it does.
    let network = "zQmTwoSeeds";
    // More than once, because the failure this guards against is a race and a race that is lost
    // one time in three is still lost.
    for round in 0..3u8 {
        let (mut one, to_one) = a_place(6, network, 0).await;
        let (mut other, to_other) = a_place(7, network, 0).await;

        one.dial(to_other.clone()).expect("somewhere to dial");
        other.dial(to_one.clone()).expect("somewhere to dial");

        let (one_met, other_met) = both_met(&mut one, &mut other).await;
        assert!(one_met && other_met, "round {round}: both should have met");

        // And the set anybody may read says the same, on both sides.
        assert!(one.peers().connected().contains(&named(7)), "round {round}");
        assert!(
            other.peers().connected().contains(&named(6)),
            "round {round}"
        );
        assert_eq!(
            one.peers().count(),
            1,
            "round {round}: one peer, however many connections"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_node_that_went_away_is_dialled_again_when_it_comes_back() {
    // **The ordinary reason a connection ends is that the other node restarted.** A node that
    // noticed and did nothing would be reachable afterwards only by whoever happened to dial it —
    // and two seeds that both restarted would each wait for the other for ever.
    let government = key(5);
    let holder = Node::open(&at(), &[], &government, key(6)).expect("nobody to join");
    let joiner = Node::open(&at(), &[], &government, key(7)).expect("nobody to join");
    let network = holder.network().as_str().to_owned();
    let joiner = Arc::new(RwLock::new(joiner));

    let (first, to_first) = a_place(6, &network, 0).await;
    // The port of the address that was dialled, and not whichever the socket names first: told to
    // take whatever is free, it took one port for each address family.
    let Some(port) = to_first.iter().find_map(|part| match part {
        Protocol::Tcp(port) => Some(port),
        _ => None,
    }) else {
        panic!("a port to come back on")
    };
    let away = driven(first);

    let (second, _) = a_place(7, &network, 0).await;
    let peers = second.peers();
    tokio::spawn(almena_mesh::keeping::keeping_up(
        second,
        Arc::clone(&joiner),
        vec![to_first.clone()],
        || Epoch::GENESIS,
        OFTEN,
    ));
    assert!(
        until(|| async { peers.connected().contains(&named(6)) }).await,
        "the second reached the first"
    );

    // The first goes: its socket is dropped, which is what a process ending does.
    away.abort();
    let _ = away.await;
    assert!(
        until(|| async { !peers.connected().contains(&named(6)) }).await,
        "and the second noticed"
    );

    // And comes back on the same port, as a restarted node does.
    let (back, _) = a_place(6, &network, port).await;
    let _still = driven(back);
    assert!(
        until(|| async { peers.connected().contains(&named(6)) }).await,
        "the second dialled the first again, without anybody telling it to"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_node_holding_the_record_dials_whoever_it_says_can_be_reached() {
    // **No zone, no seeds, and still a network.** The record names everybody who ever said where
    // they were. A node that holds it and still waited for a zone to name somebody would be a
    // node that could not come back up without DNS — and a whole network restarting would be a
    // whole network waiting.
    let government = key(5);
    let mut holder = Node::open(&at(), &[], &government, key(6)).expect("nobody to join");
    let mut joiner = Node::open(&at(), &[], &government, key(7)).expect("nobody to join");
    let network = holder.network().as_str().to_owned();

    // The first says in the record where it is — without its identity on the end, as a face
    // publishing a host and a port would.
    let (first, to_first) = a_place(6, &network, 0).await;
    let mut said = to_first.clone();
    said.pop();
    assert!(
        holder.also_reachable_at(
            &std::collections::BTreeSet::from([said.to_string()]),
            Epoch::GENESIS
        ),
        "a node may say where it is"
    );

    // The second holds the first's record, and nothing else: no seed and no zone.
    handed_over(&holder, &mut joiner);
    assert!(
        !joiner.reachable_at(holder.did().name()).is_empty(),
        "the second's record says where the first is"
    );

    let holder = Arc::new(RwLock::new(holder));
    let joiner = Arc::new(RwLock::new(joiner));
    tokio::spawn(almena_mesh::keeping::keeping_up(
        first,
        Arc::clone(&holder),
        Vec::new(),
        || Epoch::GENESIS,
        OFTEN,
    ));

    let (second, _) = a_place(7, &network, 0).await;
    let peers = second.peers();
    tokio::spawn(almena_mesh::keeping::keeping_up(
        second,
        Arc::clone(&joiner),
        Vec::new(),
        || Epoch::GENESIS,
        OFTEN,
    ));

    assert!(
        until(|| async { peers.connected().contains(&named(6)) }).await,
        "the second found the first through the record alone"
    );
}
