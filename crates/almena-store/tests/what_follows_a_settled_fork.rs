//! A fork somebody settled, and the acts that come after it.
//!
//! **A branch that lost is set aside, and nothing carries on from it as though it had landed.** A
//! resolution keeps one branch and leaves the others in the record without effect; an act chained
//! from one of those is not extending the object, whatever its `prev` says — it is splitting it
//! again, and it has to do so on every node that has it. Read off the followed acts alone it looked
//! like an ordinary extension, and only the nodes that happened to hold the losing branch applied
//! it: the sharpest divergence a store can produce, and one nobody had to lie for.
//!
//! And a chain that splits again says so: the object answers *forked again*, naming the act that
//! settled it last time, so that whoever signs the next resolution is offered the branch they kept.

#![allow(clippy::expect_used, reason = "the whole of this file is a test")]

use std::collections::BTreeMap;

use almena_format::cbor::Value;
use almena_format::identifier::{Did, Name, Network};
use almena_format::operation::{Operation, Signed, create};
use almena_store::chain::{Admitted, Answer, Objects, Reason, Refused, State};
use almena_store::kind::Kind;
use almena_store::resolution;
use almena_suite::{ed25519, p256};
use almena_time::Epoch;

/// After everything the words alone asked for has landed.
fn settled() -> Epoch {
    Epoch::new(almena_time::deadline::CONTROL_KEY_WAIT.now() + 1)
}

fn words(seed: u8) -> ed25519::SigningKey {
    ed25519::SigningKey::from_secret([seed; 32])
}

fn device(seed: u8) -> p256::SigningKey {
    p256::SigningKey::from_secret([seed; 32]).expect("a valid scalar")
}

fn by_the_words(operation: &mut Operation, control: &ed25519::SigningKey) {
    let public = control.verifying_key().bytes();
    let signature = control.sign(&operation.signing_bytes());
    operation.signatures.push(Signed {
        by: operation.object.clone(),
        key: public.to_vec(),
        signature: signature.bytes(),
    });
}

fn by_a_device(operation: &mut Operation, holds: &p256::SigningKey) {
    let over = operation.signing_bytes();
    operation.signatures.push(Signed {
        by: operation.object.clone(),
        key: holds.verifying_key().bytes().to_vec(),
        signature: holds.sign(&over).bytes(),
    });
}

fn creation(control: u8) -> Operation {
    let public = words(control).verifying_key().bytes();
    let mut created = create(
        Network::Development,
        Kind::HOLDER_CREATE.number(),
        1,
        Epoch::GENESIS,
        BTreeMap::from([(1, Value::Bytes(public.to_vec()))]),
    );
    by_the_words(&mut created, &words(control));
    created
}

fn following(whose: &Did, head: &Name, kind: Kind, payload: BTreeMap<u64, Value>) -> Operation {
    Operation {
        object: whose.clone(),
        previous: Some(head.clone()),
        kind: kind.number(),
        version: 1,
        issued: settled(),
        payload,
        signatures: Vec::new(),
    }
}

/// Device 11 adds another device, chained from `head`.
fn device_adds(whose: &Did, head: &Name, holds: u8) -> Operation {
    let mut adding = following(
        whose,
        head,
        Kind::HOLDER_ADD_DEVICE,
        BTreeMap::from([(
            1,
            Value::Bytes(device(holds).verifying_key().bytes().to_vec()),
        )]),
    );
    by_a_device(&mut adding, &device(11));
    adding
}

/// The words rotate themselves, chained from `head`.
fn words_rotate(whose: &Did, head: &Name, to: u8) -> Operation {
    let mut rotating = following(
        whose,
        head,
        Kind::HOLDER_ROTATE,
        BTreeMap::from([(1, Value::Bytes(words(to).verifying_key().bytes().to_vec()))]),
    );
    by_the_words(&mut rotating, &words(1));
    rotating
}

/// Device 11 settles the fork by chaining from `keeping` and saying so.
fn device_settles(whose: &Did, keeping: &Name, holds: u8) -> Operation {
    let mut settling = following(
        whose,
        keeping,
        Kind::HOLDER_ADD_DEVICE,
        BTreeMap::from([(
            1,
            Value::Bytes(device(holds).verifying_key().bytes().to_vec()),
        )]),
    );
    resolution::declaring(&mut settling, keeping);
    by_a_device(&mut settling, &device(11));
    settling
}

/// A fork and its settlement: the acts, in the order a node that saw everything would take them.
struct Story {
    whose: Did,
    /// The creation and the words adding device 11.
    opening: Vec<Operation>,
    /// Device 11 adds device 12, from the head. The branch that will be kept.
    kept: Operation,
    /// The words rotate, from the same head. The branch that will lose.
    lost: Operation,
    /// Device 11 chains from `kept` and declares it kept.
    settling: Operation,
}

fn a_story() -> Story {
    let created = creation(1);
    let whose = created.object.clone();
    let mut first = following(
        &whose,
        &created.called(),
        Kind::HOLDER_ADD_DEVICE,
        BTreeMap::from([(1, Value::Bytes(device(11).verifying_key().bytes().to_vec()))]),
    );
    first.issued = Epoch::GENESIS;
    by_the_words(&mut first, &words(1));
    let head = first.called();

    let kept = device_adds(&whose, &head, 12);
    let lost = words_rotate(&whose, &head, 9);
    let settling = device_settles(&whose, &kept.called(), 13);
    Story {
        whose,
        opening: vec![created, first],
        kept,
        lost,
        settling,
    }
}

fn holding(objects: &mut Objects, acts: &[&Operation]) {
    for act in acts {
        objects.admit(act, settled()).expect("taken");
    }
}

/// Settle the fork the way a node does: admit the resolution, then replay the branch it named.
fn settle(objects: &mut Objects, story: &Story) {
    assert_eq!(
        objects.admit(&story.settling, settled()),
        Ok(Admitted::Resolves)
    );
    let mut along: Vec<Operation> = story.opening.clone();
    along.push(story.kept.clone());
    along.push(story.settling.clone());
    objects
        .resolved(story.whose.name(), &along, settled())
        .expect("the branch it named, replayed");
}

/// A node that held both branches when the fork was settled.
fn saw_both_branches(story: &Story) -> Objects {
    let mut objects = Objects::new();
    holding(&mut objects, &[&story.opening[0], &story.opening[1]]);
    assert_eq!(
        objects.admit(&story.kept, settled()),
        Ok(Admitted::Extended)
    );
    assert_eq!(objects.admit(&story.lost, settled()), Ok(Admitted::Forked));
    settle(&mut objects, story);
    objects
}

/// A node that never heard of the losing branch.
///
/// To it the resolution is an ordinary act on an object that never split: there is nothing to
/// replay, and it is taken as any other act is.
fn never_saw_the_loser(story: &Story) -> Objects {
    let mut objects = Objects::new();
    holding(&mut objects, &[&story.opening[0], &story.opening[1]]);
    assert_eq!(
        objects.admit(&story.kept, settled()),
        Ok(Admitted::Extended)
    );
    assert_eq!(
        objects.admit(&story.settling, settled()),
        Ok(Admitted::Extended),
        "nothing to settle here"
    );
    objects
}

fn how_many_devices(objects: &Objects, whose: &Did) -> usize {
    match objects.resolve(whose.name()) {
        Answer::Here(State::Holder(holder)) => holder.come_due(settled()).devices.len(),
        other => panic!("{other:?}"),
    }
}

#[test]
fn settling_leaves_both_nodes_holding_the_branch_that_was_kept() {
    let story = a_story();
    let both = saw_both_branches(&story);
    let one = never_saw_the_loser(&story);
    assert_eq!(how_many_devices(&both, &story.whose), 3);
    assert_eq!(how_many_devices(&one, &story.whose), 3);
    assert_eq!(
        both.head(story.whose.name()),
        Some(&story.settling.called())
    );
}

#[test]
fn an_act_chained_from_a_losing_act_splits_the_object_again_rather_than_extending_it() {
    // The divergence this exists to close: before, the node that held the losing branch applied
    // this act to the kept branch's state and moved its head onto it, and the node that did not
    // refused it. Now the first splits and the second refuses — and once the second has the
    // losing branch, it splits too.
    let story = a_story();
    let mut both = saw_both_branches(&story);
    let mut one = never_saw_the_loser(&story);
    let on_lost = device_adds(&story.whose, &story.lost.called(), 14);

    assert_eq!(both.admit(&on_lost, settled()), Ok(Admitted::Forked));
    assert_eq!(
        both.resolve(story.whose.name()),
        Answer::CannotResolve(Reason::ForkedAgain(story.settling.called())),
        "forked again, and it says which settlement it undoes"
    );
    assert_eq!(
        both.head(story.whose.name()),
        Some(&story.settling.called()),
        "the kept branch's head did not move"
    );

    assert_eq!(
        one.admit(&on_lost, settled()),
        Err(Refused::NoSuchPredecessor)
    );
    assert_eq!(how_many_devices(&one, &story.whose), 3);
}

#[test]
fn the_losing_branch_arriving_after_the_settlement_says_it_was_settled_before() {
    // On the node that never saw it, the losing act is a second act on a followed predecessor —
    // a fork — and what the object answers names the settlement rather than looking like a first
    // split. After that the act chained from it forks here exactly as it did on the other node.
    let story = a_story();
    let mut one = never_saw_the_loser(&story);

    assert_eq!(one.admit(&story.lost, settled()), Ok(Admitted::Forked));
    assert_eq!(
        one.resolve(story.whose.name()),
        Answer::CannotResolve(Reason::ForkedAgain(story.settling.called()))
    );
    let on_lost = device_adds(&story.whose, &story.lost.called(), 14);
    assert_eq!(one.admit(&on_lost, settled()), Ok(Admitted::Forked));
}

#[test]
fn a_never_seen_branch_from_the_fork_point_says_which_settlement_it_undoes() {
    // The case as it was already known: honest on both nodes, and now no longer byte-identical
    // to a first fork.
    let story = a_story();
    let mut both = saw_both_branches(&story);
    let mut one = never_saw_the_loser(&story);
    let third = device_adds(&story.whose, &story.opening[1].called(), 15);

    for objects in [&mut both, &mut one] {
        assert_eq!(objects.admit(&third, settled()), Ok(Admitted::Forked));
        assert_eq!(
            objects.resolve(story.whose.name()),
            Answer::CannotResolve(Reason::ForkedAgain(story.settling.called()))
        );
    }
}

#[test]
fn an_act_on_the_kept_branch_still_extends_it() {
    // Setting the losers aside must not touch what was kept.
    let story = a_story();
    let mut both = saw_both_branches(&story);
    let onwards = device_adds(&story.whose, &story.settling.called(), 16);
    assert_eq!(both.admit(&onwards, settled()), Ok(Admitted::Extended));
    assert_eq!(how_many_devices(&both, &story.whose), 4);
}

#[test]
fn settling_again_keeps_what_was_kept_and_what_lost_stays_set_aside() {
    // A second resolution, chained from the first, settles the second split. What lost the first
    // time is still set aside afterwards: an act chained from it splits the object a third time,
    // naming the second settlement now.
    let story = a_story();
    let mut both = saw_both_branches(&story);
    let on_lost = device_adds(&story.whose, &story.lost.called(), 14);
    assert_eq!(both.admit(&on_lost, settled()), Ok(Admitted::Forked));

    let again = device_settles(&story.whose, &story.settling.called(), 17);
    assert_eq!(both.admit(&again, settled()), Ok(Admitted::Resolves));
    let mut along = story.opening.clone();
    along.push(story.kept.clone());
    along.push(story.settling.clone());
    along.push(again.clone());
    both.resolved(story.whose.name(), &along, settled())
        .expect("the branch it named, replayed");
    assert_eq!(how_many_devices(&both, &story.whose), 4);

    let still_lost = device_adds(&story.whose, &story.lost.called(), 18);
    assert_eq!(both.admit(&still_lost, settled()), Ok(Admitted::Forked));
    assert_eq!(
        both.resolve(story.whose.name()),
        Answer::CannotResolve(Reason::ForkedAgain(again.called()))
    );
}

#[test]
fn an_act_from_a_losing_act_is_judged_against_what_that_act_left() {
    // The losing branch rotated the words to key 9, and what the words ask alone lands after its
    // wait. Once it has, an act from that branch signed by the new words is entitled *there* — and
    // splits the object — while one signed by the old words is a stranger there, whatever the
    // kept branch, which never rotated, says about them.
    let story = a_story();
    let mut both = saw_both_branches(&story);
    let once_landed =
        Epoch::new(settled().number() + almena_time::deadline::CONTROL_KEY_WAIT.now());

    let mut by_old_words = following(
        &story.whose,
        &story.lost.called(),
        Kind::HOLDER_ADD_DEVICE,
        BTreeMap::from([(1, Value::Bytes(device(19).verifying_key().bytes().to_vec()))]),
    );
    by_old_words.issued = once_landed;
    by_the_words(&mut by_old_words, &words(1));
    assert_eq!(
        both.admit(&by_old_words, once_landed),
        Err(Refused::NotAuthorised)
    );

    let mut by_new_words = following(
        &story.whose,
        &story.lost.called(),
        Kind::HOLDER_ADD_DEVICE,
        BTreeMap::from([(1, Value::Bytes(device(19).verifying_key().bytes().to_vec()))]),
    );
    by_new_words.issued = once_landed;
    by_the_words(&mut by_new_words, &words(9));
    assert_eq!(both.admit(&by_new_words, once_landed), Ok(Admitted::Forked));
}
