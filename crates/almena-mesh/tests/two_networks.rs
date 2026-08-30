//! A development node and a production node, on one machine, that cannot say a word to each other.
//!
//! **The check `SPECS.md §4.5` asks for and `SPECS.md §4.12` answers.** Reading a different zone
//! cannot be the only thing that separates two networks: a development node and a production node
//! that learn each other's address — through a cache, a copied record, a deployment pointed at the
//! wrong place — would merge, and in an append-only record that does not come apart again.
//!
//! So which network a node is on rides **inside the name of the protocol** it offers:
//!
//! ```text
//! /almena/<the hash of the act that opened the network>/sync/1.0.0
//! ```
//!
//! What that buys is the thing this file exists to demonstrate: **it is not a check anybody can
//! forget to implement**. There is no field to compare and no branch to leave out. Two nodes on two
//! networks connect at the transport, find nothing in common, and the question one of them put
//! comes back as one that will not be answered.
//!
//! And it separates two **production** networks as well, not only production from development. The
//! label inside the genesis says *production* in both, so the label cannot tell them apart; the
//! hash of the act that opened each of them can, because there are not two of those.

#![allow(clippy::expect_used, reason = "the whole of this file is a test")]

use almena_mesh::sync::Ask;
use almena_mesh::{Happened, Listening, Unanswerable};
use almena_node::{Epoch, Node, Opening, SigningKey, Which};

/// How long anything here is given before it is called a failure.
const PATIENCE: std::time::Duration = std::time::Duration::from_secs(20);

/// A key nobody else in this test has.
fn key(seed: u8) -> SigningKey {
    SigningKey::from_secret([seed; 32])
}

/// A network of that kind, with a fixed clock so a test is never about what time it is here.
fn a_network(which: Which, government: u8, node: u8) -> Node {
    Node::open(
        &Opening {
            which,
            beginning: Epoch::GENESIS,
            began: 1_800_000_000,
        },
        &[],
        &key(government),
        key(node),
    )
    .expect("nobody to join")
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

/// Drive both until the one that asked finds out it will get nothing.
///
/// They are driven together because each has to keep answering while the other is asking — and the
/// point of the test is that one of them has nothing it *could* answer.
async fn nothing_in_common(theirs: &mut Listening, ours: &mut Listening) -> Unanswerable {
    let mut asked = false;
    loop {
        tokio::select! {
            _ = theirs.next() => {}
            happened = ours.next() => match happened {
                Happened::Met(peer, _) if !asked => {
                    asked = true;
                    ours.ask(&peer, Ask::Since(0));
                }
                Happened::Unanswered(_, _, why) => return why,
                Happened::Answered(_, _, said) => {
                    panic!("a node on another network answered with {} acts", said.acts.len())
                }
                _ => {}
            },
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_development_node_and_a_production_node_have_nothing_to_say_to_each_other() {
    // The accident `SPECS.md §4.5` calls the one that costs the most, and what stops it. Both nodes
    // are real, both are listening, and one dials the other on an address it was handed.
    let development = a_network(Which::Development, 5, 6);
    let production = a_network(Which::Production, 7, 8);
    assert_ne!(development.network(), production.network());

    let mut theirs =
        almena_mesh::listen(&key(6), development.network().as_str(), 0).expect("a place to listen");
    let mut ours =
        almena_mesh::listen(&key(8), production.network().as_str(), 0).expect("a place to listen");

    let address = tokio::time::timeout(PATIENCE, reachable(&mut theirs))
        .await
        .expect("the development node should be listening");
    tokio::time::timeout(PATIENCE, reachable(&mut ours))
        .await
        .expect("the production node should be listening");

    ours.dial(address).expect("dialled");
    let why = tokio::time::timeout(PATIENCE, nothing_in_common(&mut theirs, &mut ours))
        .await
        .expect("the question should come back unanswerable");

    assert_eq!(
        why,
        Unanswerable::NotOnThisNetwork,
        "the negotiation is what fails, and it fails without anybody comparing a field"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn two_production_networks_are_two_networks_and_the_label_would_not_have_said_so() {
    // The consequence that matters more than it looks. Two production networks opened by accident
    // say exactly the same word about themselves — the genesis label is *production* in both — so
    // if the label were what separated networks, these two would merge. The hash of the act that
    // opened each of them is what actually keeps them apart.
    let one = a_network(Which::Production, 11, 12);
    let other = a_network(Which::Production, 13, 14);

    // Composed again here rather than read off a node, because what is being shown is the act
    // itself: two of them, both saying the word *production*, with two different hashes.
    let said = |government: u8| {
        let opened = almena_store::genesis::open(
            &Opening {
                which: Which::Production,
                beginning: Epoch::GENESIS,
                began: 1_800_000_000,
            },
            &[],
            false,
            &key(government),
        )
        .expect("nobody to join");
        almena_store::genesis::declares(&opened.operation)
    };
    assert_eq!(
        said(11),
        said(13),
        "both say production, which is why the label is no use"
    );
    assert_ne!(one.network(), other.network());

    let mut theirs =
        almena_mesh::listen(&key(12), one.network().as_str(), 0).expect("a place to listen");
    let mut ours =
        almena_mesh::listen(&key(14), other.network().as_str(), 0).expect("a place to listen");

    let address = tokio::time::timeout(PATIENCE, reachable(&mut theirs))
        .await
        .expect("the first node should be listening");
    tokio::time::timeout(PATIENCE, reachable(&mut ours))
        .await
        .expect("the second node should be listening");

    ours.dial(address).expect("dialled");
    let why = tokio::time::timeout(PATIENCE, nothing_in_common(&mut theirs, &mut ours))
        .await
        .expect("the question should come back unanswerable");

    assert_eq!(why, Unanswerable::NotOnThisNetwork);
}

#[test]
fn the_network_is_in_the_name_of_the_protocol_and_not_beside_it() {
    // Said plainly, because it is the whole reason the two tests above pass without any code in
    // this repository comparing anything: what a node offers is named after the act that opened its
    // network, so a node on another network offers a name this one never asks for.
    let development = a_network(Which::Development, 5, 6);
    let production = a_network(Which::Production, 7, 8);

    let ours = almena_mesh::syncing(production.network().as_str());
    let theirs = almena_mesh::syncing(development.network().as_str());

    assert_ne!(ours, theirs);
    assert!(ours.starts_with("/almena/"));
    assert!(ours.contains(production.network().as_str()));
    assert!(ours.ends_with("/sync/1.0.0"));
}
