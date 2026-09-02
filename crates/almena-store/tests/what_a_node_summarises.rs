//! A node's own chain, summarised by the node that owns it.
//!
//! A node's chain grows by one daily summary for as long as it runs, plus an announcement every
//! time what it offers changes, and nothing ever shortens it: after a month it owes a summary like
//! any object that has written that much, and until its state had parts a summary could claim it
//! could never pay. What a node **is** — the key it signs with, what it says it is running, where
//! it says it can be reached, whether it has closed — is what its announcements and its closing
//! set, and this walks a node summarising that and a reader checking it against the record.

#![allow(clippy::expect_used, reason = "the whole of this file is a test")]

use std::collections::{BTreeMap, BTreeSet};

use almena_format::cbor::Value;
use almena_format::identifier::{Did, Name};
use almena_format::operation::{Operation, Signed};
use almena_store::announce;
use almena_store::capability::Capability;
use almena_store::chain::{Admitted, Answer, Objects, State};
use almena_store::checkpoint::{self, Governs, Stated, Verdict};
use almena_store::genesis::Which;
use almena_store::kind::Kind;
use almena_suite::ed25519;
use almena_time::Epoch;

fn key() -> ed25519::SigningKey {
    ed25519::SigningKey::from_secret([5; 32])
}

/// An act on the node's chain, signed by the node.
fn signed(mut operation: Operation) -> Operation {
    let signature = key().sign(&operation.signing_bytes());
    operation.signatures.push(Signed {
        by: operation.object.clone(),
        key: key().verifying_key().bytes().to_vec(),
        signature: signature.bytes(),
    });
    operation
}

/// A daily summary as the mesh writes one, carrying whatever else it is given.
fn a_daily_summary(node: &Did, head: &Name, at: Epoch, also: BTreeMap<u64, Value>) -> Operation {
    let mut payload = BTreeMap::from([
        (1, Value::Uint(at.number() / 24)),
        (3, Value::Bytes(vec![0; 32])),
        (5, Value::Array(Vec::new())),
        (7, Value::Array(vec![Value::Uint(0), Value::Uint(0)])),
    ]);
    payload.extend(also);
    signed(Operation {
        object: node.clone(),
        previous: Some(head.clone()),
        kind: Kind::NODE_SUMMARY.number(),
        version: 1,
        issued: at,
        payload,
        signatures: Vec::new(),
    })
}

/// A node announced, offering the interface at one address, and its entries so far.
fn a_node() -> (Objects, Did, Vec<Operation>) {
    let mut objects = Objects::new();
    let announced = announce::announce(Which::Development, Epoch::GENESIS, &key());
    objects
        .admit(&announced.operation, Epoch::GENESIS)
        .expect("announced");
    let node = announced.node;

    let offering = announce::offering(
        &node,
        &announced.operation.called(),
        &BTreeSet::from([Capability::Interface]),
        announce::Speaking {
            version: 1,
            reachable: &BTreeSet::from(["/ip4/10.0.0.1/tcp/4001".to_owned()]),
            issued: Epoch::new(1),
            key: &key(),
        },
    );
    objects.admit(&offering, Epoch::new(1)).expect("offered");
    (objects, node, vec![announced.operation, offering])
}

/// The entries a node would hold for these acts, in the order it wrote them.
fn entries(acts: &[Operation]) -> Vec<almena_format::entry::Entry> {
    acts.iter()
        .enumerate()
        .map(|(at, act)| almena_format::entry::Entry::of(act, at as u64, None))
        .collect()
}

#[test]
fn a_node_s_chain_has_parts_a_summary_can_claim() {
    let (objects, node, acts) = a_node();
    let standing = objects
        .standing(node.name(), Epoch::new(1))
        .expect("a node's chain resolves and has parts");
    let stated = |about: Governs| {
        standing
            .claims
            .iter()
            .find(|claim| claim.about == about)
            .map(|claim| (claim.stated.clone(), claim.set_by.clone()))
    };
    assert_eq!(
        stated(Governs::NodeKey),
        Some((
            Stated::Key(key().verifying_key().bytes().to_vec()),
            acts[1].called()
        )),
        "the key, cited to the latest announcement"
    );
    assert_eq!(
        stated(Governs::Offers),
        Some((
            Stated::Numbers(BTreeSet::from([Capability::Interface.number()])),
            acts[1].called()
        ))
    );
    assert_eq!(
        stated(Governs::Reachable),
        Some((
            Stated::Addresses(BTreeSet::from(["/ip4/10.0.0.1/tcp/4001".to_owned()])),
            acts[1].called()
        ))
    );
    assert_eq!(
        stated(Governs::Closed),
        Some((Stated::Moment(None), acts[1].called()))
    );
    assert_eq!(standing.since, 2, "two acts and no summary yet");
}

#[test]
fn the_summary_a_node_offers_rides_on_its_daily_summary_and_stands_up() {
    // The summary of the node's own state travels in the same field on any act, and the act a
    // node writes every day is the natural carrier: nothing extra is signed and nothing waits.
    let (mut objects, node, mut acts) = a_node();
    let standing = objects
        .standing(node.name(), Epoch::new(24))
        .expect("it resolves");
    let carrying = a_daily_summary(
        &node,
        &acts[1].called(),
        Epoch::new(24),
        BTreeMap::from([(checkpoint::FIELD, checkpoint::declaration(&standing.claims))]),
    );
    assert_eq!(
        objects.admit(&carrying, Epoch::new(24)),
        Ok(Admitted::Extended)
    );
    acts.push(carrying.clone());
    assert!(
        matches!(
            objects.resolve(node.name()),
            Answer::Here(State::Node { .. })
        ),
        "and the node is exactly what it was"
    );
    assert_eq!(
        objects
            .standing(node.name(), Epoch::new(24))
            .expect("it resolves")
            .since,
        0,
        "the summary put the count back to nothing"
    );

    // A reader holding the record checks it: nothing hidden, and the values are what the acts
    // that set them produce.
    let held = entries(&acts);
    let held: Vec<&almena_format::entry::Entry> = held.iter().collect();
    let carrier = carrying.called();
    let walked = checkpoint::branch(&held, &carrier);
    let placed = checkpoint::Placed {
        carrier: &carrier,
        branch: &walked,
    };
    let claims = checkpoint::declared(&carrying)
        .expect("readable")
        .expect("a summary");
    let fetched: Vec<&Operation> = acts.iter().collect();
    assert!(
        checkpoint::falls_over(&claims, placed, &fetched, Epoch::new(24)).is_empty(),
        "it stands"
    );
}

/// What a reader holding the record makes of a summary carried by the last of these acts.
fn checked(acts: &[Operation], claims: &[checkpoint::Claim], at: Epoch) -> Vec<(Governs, Verdict)> {
    let held = entries(acts);
    let held: Vec<&almena_format::entry::Entry> = held.iter().collect();
    let carrier = acts.last().expect("an act").called();
    let walked = checkpoint::branch(&held, &carrier);
    let placed = checkpoint::Placed {
        carrier: &carrier,
        branch: &walked,
    };
    let fetched: Vec<&Operation> = acts.iter().collect();
    checkpoint::falls_over(claims, placed, &fetched, at)
}

/// The node again, after saying it offers more and then closing: the record, its acts, the claims
/// that were true before either, and the two acts.
fn a_node_that_grew_and_closed() -> (
    Objects,
    Did,
    Vec<Operation>,
    Vec<checkpoint::Claim>,
    Operation,
    Operation,
) {
    let (mut objects, node, mut acts) = a_node();
    let stale = objects
        .standing(node.name(), Epoch::new(1))
        .expect("it resolves")
        .claims;
    let again = announce::offering(
        &node,
        &acts[1].called(),
        &BTreeSet::from([Capability::Interface, Capability::Mailbox]),
        announce::Speaking {
            version: 1,
            reachable: &BTreeSet::from(["/ip4/10.0.0.1/tcp/4001".to_owned()]),
            issued: Epoch::new(2),
            key: &key(),
        },
    );
    objects.admit(&again, Epoch::new(2)).expect("offered again");
    acts.push(again.clone());
    let closing = announce::close(&node, &again.called(), Epoch::new(3), &key());
    objects.admit(&closing, Epoch::new(3)).expect("closed");
    acts.push(closing.clone());
    (objects, node, acts, stale, again, closing)
}

#[test]
fn a_node_summary_that_hides_an_announcement_and_the_closing_falls_over() {
    let (mut objects, node, mut acts, stale, again, closing) = a_node_that_grew_and_closed();

    // A summary carrying the stale claims: the first announcement cited for what it offers, and
    // nothing said about closing.
    let carrying = a_daily_summary(
        &node,
        &closing.called(),
        Epoch::new(24),
        BTreeMap::from([(checkpoint::FIELD, checkpoint::declaration(&stale))]),
    );
    objects.admit(&carrying, Epoch::new(24)).expect("kept");
    acts.push(carrying);

    assert_eq!(
        checked(&acts, &stale, Epoch::new(24)),
        vec![
            (Governs::NodeKey, Verdict::LeftOut(again.called())),
            (Governs::Offers, Verdict::LeftOut(again.called())),
            (Governs::Reachable, Verdict::LeftOut(again.called())),
            (Governs::Closed, Verdict::LeftOut(again.called())),
        ],
        "every part cites an announcement a later one superseded"
    );
}

#[test]
fn the_honest_summary_of_a_closed_node_cites_the_closing_and_stands() {
    let (mut objects, node, mut acts, _stale, _again, closing) = a_node_that_grew_and_closed();
    let honest = objects
        .standing(node.name(), Epoch::new(48))
        .expect("a closed node still resolves")
        .claims;
    assert_eq!(
        honest
            .iter()
            .find(|claim| claim.about == Governs::Closed)
            .map(|claim| (&claim.stated, &claim.set_by)),
        Some((&Stated::Moment(Some(Epoch::new(3))), &closing.called()))
    );

    let carrying = a_daily_summary(
        &node,
        &closing.called(),
        Epoch::new(48),
        BTreeMap::from([(checkpoint::FIELD, checkpoint::declaration(&honest))]),
    );
    objects.admit(&carrying, Epoch::new(48)).expect("taken");
    acts.push(carrying);
    assert!(checked(&acts, &honest, Epoch::new(48)).is_empty());
}
