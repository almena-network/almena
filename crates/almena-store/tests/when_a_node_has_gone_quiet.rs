//! A node nobody can reach, and the share it is no longer dealt.
//!
//! **Read off the record, so that everybody reads the same thing.** The census names every node
//! that ever announced itself, and a share dealt to a node that has gone is a copy that does not
//! exist. What the record holds about that is the daily summaries — other nodes, who gain nothing
//! by it, wrote down whether it answered — and a node every observer that asked found silent over
//! the last three days is left out of the share-out. Out of the share-out only: what it said
//! stays said, its roots are still read, and the capacity figures go on counting it, as silent.

#![allow(clippy::expect_used, reason = "the whole of this file is a test")]

use std::collections::BTreeMap;

use almena_format::identifier::{Did, Name};
use almena_store::announce::{Announced, announce};
use almena_store::chain::Objects;
use almena_store::genesis::Which;
use almena_store::parameter::DEPARTED_AFTER;
use almena_store::summary::{Looked, Observer, Seen, publish};
use almena_suite::digest::Digest;
use almena_suite::ed25519;
use almena_time::{Day, EPOCHS_PER_DAY, Epoch};

fn key(seed: u8) -> ed25519::SigningKey {
    ed25519::SigningKey::from_secret([seed; 32])
}

/// Long after every summary in these tests was written.
fn much_later() -> Epoch {
    Epoch::new(1_000)
}

/// Three nodes, announced: two that watch and one that is watched.
fn three_nodes() -> (Objects, [Announced; 3]) {
    let mut objects = Objects::new();
    let nodes = [1, 2, 3].map(|seed| announce(Which::Development, Epoch::GENESIS, &key(seed)));
    for node in &nodes {
        objects
            .admit(&node.operation, Epoch::GENESIS)
            .expect("a node introducing itself");
    }
    (objects, nodes)
}

fn seen(asked: u64, answered: u64) -> Seen {
    Seen {
        asked,
        answered,
        behind: 0,
    }
}

/// What one observer wrote down about one node on one day, as the act on the observer's chain.
fn summary(
    objects: &Objects,
    observer: (u8, &Did),
    day: Day,
    about: &Did,
    saw: Seen,
) -> almena_format::operation::Operation {
    let (seed, who) = observer;
    let head = objects.head(who.name()).expect("an announced node").clone();
    publish(
        Observer {
            observer: who,
            head: &head,
            by: &key(seed),
        },
        day,
        &BTreeMap::from([(about.clone(), saw)]),
        Looked::default(),
        Digest::of(b"the observations"),
    )
    .operation
}

fn said(objects: &mut Objects, observer: (u8, &Did), day: Day, about: &Did, saw: Seen) {
    let act = summary(objects, observer, day, about, saw);
    objects.admit(&act, much_later()).expect("a summary");
}

fn census(objects: &Objects, at: Epoch) -> Vec<Name> {
    objects.nodes_at(at).cloned().collect()
}

#[test]
fn a_node_every_observer_found_silent_leaves_the_share_out_and_nothing_else() {
    let (mut objects, [one, two, quiet]) = three_nodes();
    said(
        &mut objects,
        (1, &one.node),
        Day::new(0),
        &quiet.node,
        seen(5, 0),
    );
    said(
        &mut objects,
        (2, &two.node),
        Day::new(0),
        &quiet.node,
        seen(3, 0),
    );

    let at = Epoch::new(30);
    assert!(
        !census(&objects, at).contains(quiet.node.name()),
        "not dealt a share it cannot serve"
    );
    assert_eq!(census(&objects, at).len(), 2);
    assert_eq!(
        objects.nodes().count(),
        3,
        "everything it said stays in the record"
    );
    assert_eq!(
        objects.departed_at(at).cloned().collect::<Vec<_>>(),
        vec![quiet.node.name().clone()],
        "and it is counted, as silent"
    );
    assert_eq!(
        objects.running().speaking.get(&0),
        Some(&3),
        "the capacity figures still count it"
    );
}

#[test]
fn one_observer_is_evidence_enough_and_one_answer_seen_by_anybody_is_not_silence() {
    let (mut objects, [one, two, quiet]) = three_nodes();
    said(
        &mut objects,
        (1, &one.node),
        Day::new(0),
        &quiet.node,
        seen(5, 0),
    );
    assert!(!census(&objects, Epoch::new(30)).contains(quiet.node.name()));

    // The other observer got an answer out of it: it is there, whatever the first one saw.
    said(
        &mut objects,
        (2, &two.node),
        Day::new(0),
        &quiet.node,
        seen(5, 1),
    );
    assert!(census(&objects, Epoch::new(30)).contains(quiet.node.name()));
}

#[test]
fn an_observer_that_asked_nothing_has_no_evidence_of_silence() {
    // A node could otherwise be left out of the share-out by observers that looked the other way.
    let (mut objects, [one, _, quiet]) = three_nodes();
    said(
        &mut objects,
        (1, &one.node),
        Day::new(0),
        &quiet.node,
        seen(0, 0),
    );
    assert!(census(&objects, Epoch::new(30)).contains(quiet.node.name()));
}

#[test]
fn silence_is_old_news_once_the_window_has_passed() {
    // The window is the last three days before the moment asked about. A summary for day nought
    // is in it while any epoch of day nought is, and not after.
    let (mut objects, [one, _, quiet]) = three_nodes();
    said(
        &mut objects,
        (1, &one.node),
        Day::new(0),
        &quiet.node,
        seen(5, 0),
    );

    let window = DEPARTED_AFTER.at(Epoch::GENESIS);
    let last_epoch_in = Epoch::new(EPOCHS_PER_DAY + window - 1);
    let first_epoch_out = Epoch::new(EPOCHS_PER_DAY + window);
    assert!(!census(&objects, last_epoch_in).contains(quiet.node.name()));
    assert!(census(&objects, first_epoch_out).contains(quiet.node.name()));
}

#[test]
fn a_day_that_had_not_begun_says_nothing_about_the_time_before_it() {
    // A share-out drawn for an earlier epoch has to be the same share-out afterwards, as far as
    // the record allows: what an observer saw on day two is not evidence about day nought.
    let (mut objects, [one, _, quiet]) = three_nodes();
    said(
        &mut objects,
        (1, &one.node),
        Day::new(2),
        &quiet.node,
        seen(5, 0),
    );
    assert!(census(&objects, Epoch::new(10)).contains(quiet.node.name()));
    assert!(!census(&objects, Day::new(2).begins()).contains(quiet.node.name()));
}

#[test]
fn a_day_of_silence_keeps_it_out_for_three_days_and_a_day_of_answers_brings_it_back() {
    let (mut objects, [one, _, quiet]) = three_nodes();
    said(
        &mut objects,
        (1, &one.node),
        Day::new(0),
        &quiet.node,
        seen(5, 0),
    );
    said(
        &mut objects,
        (1, &one.node),
        Day::new(1),
        &quiet.node,
        seen(5, 0),
    );
    assert!(!census(&objects, Epoch::new(60)).contains(quiet.node.name()));

    said(
        &mut objects,
        (1, &one.node),
        Day::new(2),
        &quiet.node,
        seen(5, 5),
    );
    assert!(census(&objects, Epoch::new(72)).contains(quiet.node.name()));
}

#[test]
fn two_records_holding_the_same_summaries_read_the_same_silence() {
    // Whatever order they arrived in: the census is what the share-out is drawn from, and a
    // share-out two honest nodes drew differently would be no assignment at all.
    let (mut forwards, [one, two, quiet]) = three_nodes();
    let (mut backwards, _) = three_nodes();
    let first = summary(
        &forwards,
        (1, &one.node),
        Day::new(0),
        &quiet.node,
        seen(5, 0),
    );
    let second = summary(
        &forwards,
        (2, &two.node),
        Day::new(0),
        &quiet.node,
        seen(4, 0),
    );

    forwards.admit(&first, much_later()).expect("a summary");
    forwards.admit(&second, much_later()).expect("a summary");
    backwards.admit(&second, much_later()).expect("a summary");
    backwards.admit(&first, much_later()).expect("a summary");

    let at = Epoch::new(40);
    assert_eq!(census(&forwards, at), census(&backwards, at));
    assert!(!census(&forwards, at).contains(quiet.node.name()));
}

#[test]
fn the_window_is_a_parameter_with_a_history_and_starts_at_three_days() {
    assert_eq!(DEPARTED_AFTER.at(Epoch::GENESIS), 3 * EPOCHS_PER_DAY);
    assert_eq!(DEPARTED_AFTER.settings()[0].0, Epoch::GENESIS);
}
