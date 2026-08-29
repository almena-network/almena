//! The exit criterion of the mailbox, walked from outside.
//!
//! **Nothing here reaches inside a crate.** An issuer hands a sealed message to a node the way
//! anybody would; a device asks for its post the way the app does; and what comes back is read out
//! of the bytes, not out of a struct. A test that reached in would be a test that passes while the
//! surface people actually use is broken.

#![allow(clippy::expect_used, reason = "the whole of this file is a test")]

use std::collections::BTreeMap;

use almena_api::post;
use almena_api::{Said, State};
use almena_format::cbor::{Value, read};
use almena_format::identifier::{Did, Name, Network};
use almena_format::operation::{Signed, create};
use almena_node::{Node, Opening};
use almena_store::capability::Capability;
use almena_store::genesis::Which;
use almena_store::kind::Kind;
use almena_suite::{ed25519, p256};
use almena_time::Epoch;

/// Long enough for what the words alone asked for to have landed (`SPECS.md §11.12`).
fn settled() -> Epoch {
    Epoch::new(almena_time::deadline::CONTROL_KEY_WAIT.count() + 1)
}

fn control(seed: u8) -> ed25519::SigningKey {
    ed25519::SigningKey::from_secret([seed; 32])
}

fn device(seed: u8) -> p256::SigningKey {
    p256::SigningKey::from_secret([seed; 32]).expect("a key")
}

/// A node that has said it holds post.
fn a_mediator() -> Node {
    let mut node = Node::open(
        &Opening {
            which: Which::Development,
            beginning: Epoch::GENESIS,
            began: 1_800_000_000,
        },
        &[],
        &control(5),
        control(6),
    )
    .expect("nobody to join");
    assert!(
        node.also_offering(Capability::Mailbox, Epoch::GENESIS),
        "and it says so in the record, which is the only place it can be said"
    );
    node
}

/// An account with one device on it, as of [`settled`].
fn an_account(node: &mut Node, words: &ed25519::SigningKey, holds: &p256::SigningKey) -> Did {
    let public = words.verifying_key().bytes();
    let created = signed(
        create(
            Network::Development,
            Kind::HOLDER_CREATE.number(),
            1,
            Epoch::GENESIS,
            BTreeMap::from([(1, Value::Bytes(public.to_vec()))]),
        ),
        words,
    );
    let whose = created.object.clone();
    node.submit(&created, Epoch::GENESIS).expect("the account");

    let adding = following(
        &After {
            whose: &whose,
            kind: Kind::HOLDER_ADD_DEVICE.number(),
            at: Epoch::GENESIS,
            head: created.called(),
        },
        BTreeMap::from([(1, Value::Bytes(holds.verifying_key().bytes().to_vec()))]),
        words,
    );
    node.submit(&adding, Epoch::GENESIS).expect("the asking");
    whose
}

/// Where one act on an existing chain goes.
struct After<'a> {
    /// Whose chain.
    whose: &'a Did,
    /// Which kind of act.
    kind: u64,
    /// The epoch it is written in.
    at: Epoch,
    /// The act it follows.
    head: Name,
}

/// One act on an object's existing chain, signed by the words.
fn following(
    on: &After<'_>,
    payload: BTreeMap<u64, Value>,
    words: &ed25519::SigningKey,
) -> almena_format::operation::Operation {
    signed(
        almena_format::operation::Operation {
            object: on.whose.clone(),
            previous: Some(on.head.clone()),
            kind: on.kind,
            version: 1,
            issued: on.at,
            payload,
            signatures: Vec::new(),
        },
        words,
    )
}

fn signed(
    mut operation: almena_format::operation::Operation,
    words: &ed25519::SigningKey,
) -> almena_format::operation::Operation {
    let public = words.verifying_key().bytes();
    let signature = words.sign(&operation.signing_bytes());
    operation.signatures.push(Signed {
        by: operation.object.clone(),
        key: public.to_vec(),
        signature: signature.bytes(),
    });
    operation
}

/// What a sender writes: a relationship, sealed bytes, and how long they ask it be held.
fn envelope(relation: &str, sealed: &[u8], held_for: u64) -> Vec<u8> {
    Value::Map(BTreeMap::from([
        (1, Value::Text(relation.to_owned())),
        (3, Value::Bytes(sealed.to_vec())),
        (5, Value::Uint(held_for)),
    ]))
    .to_bytes()
}

/// One asking, signed by a device.
fn asking(
    errand: almena_mailbox::asking::Errand,
    whose: &Did,
    names: Vec<String>,
    key: &p256::SigningKey,
    at: Epoch,
) -> Vec<u8> {
    almena_mailbox::asking::Asking {
        errand,
        whose: whose.clone(),
        device: Vec::new(),
        at,
        names,
        signed: Vec::new(),
    }
    .signed_by(key)
    .to_bytes()
}

/// The one relationship these tests use, declared the way the app declares them.
fn declaring(node: &mut Node, whose: &Did, holds: &p256::SigningKey, at: Epoch) -> Said {
    post::asked(
        node,
        &asking(
            almena_mailbox::asking::Errand::Carry,
            whose,
            vec!["zTheIssuer".to_owned()],
            holds,
            at,
        ),
        at,
    )
}

/// The state a node answered with.
fn state(said: &Said) -> u64 {
    match read(&said.body) {
        Ok(Value::Map(fields)) => match fields.get(&3) {
            Some(Value::Uint(state)) => *state,
            other => panic!("a response says what happened, got {other:?}"),
        },
        other => panic!("a response is a canonical map, got {other:?}"),
    }
}

/// What a node handed back, when it handed anything back.
fn payload(said: &Said) -> BTreeMap<u64, Value> {
    match read(&said.body) {
        Ok(Value::Map(fields)) => match fields.get(&4) {
            Some(Value::Map(carried)) => carried.clone(),
            other => panic!("it carried something, got {other:?}"),
        },
        other => panic!("a response is a canonical map, got {other:?}"),
    }
}

/// Which rule a refusal named.
fn which(said: &Said) -> u64 {
    match read(&said.body) {
        Ok(Value::Map(fields)) => match fields.get(&5) {
            Some(Value::Uint(number)) => *number,
            other => panic!("a refusal says which, got {other:?}"),
        },
        other => panic!("a response is a canonical map, got {other:?}"),
    }
}

#[test]
fn an_issuer_delivers_with_the_app_closed_and_the_device_finds_it_when_it_comes_back() {
    // **The phase's exit criterion.** Nothing in this test needs the recipient to be present at the
    // moment the message is sent, which is the whole of what a mediator is for.
    let mut node = a_mediator();
    let holds = device(11);
    let whose = an_account(&mut node, &control(9), &holds);
    let at = settled();

    // The app declares its relationships once, so that its counterparties have a floor of their own.
    assert_eq!(
        state(&declaring(&mut node, &whose, &holds, at)),
        State::Taken as u64
    );

    // Now the app is closed. An issuer writes to the address it was given.
    let sealed = b"sealed between the two ends, and never opened here";
    let left = post::deliver(
        &mut node,
        &whose.to_string(),
        &envelope("zTheIssuer", sealed, 24),
        at,
    );
    assert_eq!(state(&left), State::Taken as u64);
    assert_eq!(
        payload(&left).get(&3),
        Some(&Value::Uint(1)),
        "into a mailbox, because it is a relationship this account has"
    );

    and_the_device_coming_back_finds_it(&mut node, &whose, &holds, at, sealed);
}

/// The second half of the test above, which is one function only because of its length.
fn and_the_device_coming_back_finds_it(
    node: &mut Node,
    whose: &Did,
    holds: &p256::SigningKey,
    at: Epoch,
    sealed: &[u8],
) {
    let collected = post::asked(
        node,
        &asking(
            almena_mailbox::asking::Errand::Collect,
            whose,
            Vec::new(),
            holds,
            at,
        ),
        at,
    );
    assert_eq!(state(&collected), State::Here as u64);
    let carried = payload(&collected);
    let Some(Value::Array(waiting)) = carried.get(&1) else {
        panic!("it said what is waiting")
    };
    assert_eq!(waiting.len(), 1);
    let Value::Map(one) = &waiting[0] else {
        panic!("a message is a map")
    };
    assert_eq!(
        one.get(&5),
        Some(&Value::Bytes(sealed.to_vec())),
        "the sender's own bytes, unopened and unaltered"
    );
    assert_eq!(
        one.get(&1),
        Some(&Value::Text(Name::of(sealed).as_str().to_owned())),
        "named by its bytes, so three mediators name it the same and no sender may aim it"
    );
}

#[test]
fn a_sender_needs_the_address_and_never_the_account_it_belongs_to() {
    // **What makes a relationship possible without either end learning the other's root
    // identifier** (`SPECS.md §3.3`, `§6.5`: *the mediator already routes by peer identifier*).
    // What the sender holds is an address; which of its customers answers to it is the mediator's
    // question and nobody else's.
    let mut node = a_mediator();
    let holds = device(11);
    let whose = an_account(&mut node, &control(9), &holds);
    let at = settled();
    assert_eq!(
        state(&declaring(&mut node, &whose, &holds, at)),
        State::Taken as u64
    );

    let left = post::deliver(
        &mut node,
        "zTheIssuer",
        &envelope("zTheIssuer", b"an offer", 24),
        at,
    );
    assert_eq!(state(&left), State::Taken as u64);
    assert_eq!(
        payload(&left).get(&3),
        Some(&Value::Uint(1)),
        "into the mailboxes of the account that answers to it"
    );

    let collected = post::asked(
        &mut node,
        &asking(
            almena_mailbox::asking::Errand::Collect,
            &whose,
            Vec::new(),
            &holds,
            at,
        ),
        at,
    );
    let carried = payload(&collected);
    assert!(
        matches!(carried.get(&1), Some(Value::Array(waiting)) if waiting.len() == 1),
        "and it is there"
    );

    // An address nobody answers to is said, and not invented into somebody's mailbox.
    assert_eq!(
        state(&post::deliver(
            &mut node,
            "zNobodyAnswersToThis",
            &envelope("zNobodyAnswersToThis", b"hello", 24),
            at
        )),
        State::NotTaken as u64
    );
}

#[test]
fn somebody_with_no_relationship_rings_the_doorbell_and_is_told_so() {
    // **The doorbell is what reaches a person you have no relationship with** (`SPECS.md §6.5`),
    // and it is not a mailbox: a sender who thought they were delivering is told which of the two
    // happened rather than left to assume.
    let mut node = a_mediator();
    let holds = device(11);
    let whose = an_account(&mut node, &control(9), &holds);
    let at = settled();
    assert_eq!(
        state(&declaring(&mut node, &whose, &holds, at)),
        State::Taken as u64
    );

    let rang = post::deliver(
        &mut node,
        &whose.to_string(),
        &envelope("zNobody", b"hello", 24),
        at,
    );
    assert_eq!(state(&rang), State::Taken as u64);
    assert_eq!(
        payload(&rang).get(&3),
        Some(&Value::Uint(2)),
        "the doorbell, and said as the doorbell"
    );

    let collected = post::asked(
        &mut node,
        &asking(
            almena_mailbox::asking::Errand::Collect,
            &whose,
            Vec::new(),
            &holds,
            at,
        ),
        at,
    );
    let carried = payload(&collected);
    assert!(
        matches!(carried.get(&1), Some(Value::Array(waiting)) if waiting.is_empty()),
        "not in the mailbox"
    );
    assert!(
        matches!(carried.get(&3), Some(Value::Array(ringing)) if ringing.len() == 1),
        "at the door"
    );
}

#[test]
fn a_key_the_account_does_not_authorise_collects_nothing() {
    let mut node = a_mediator();
    let holds = device(11);
    let whose = an_account(&mut node, &control(9), &holds);
    let at = settled();

    let stranger = post::asked(
        &mut node,
        &asking(
            almena_mailbox::asking::Errand::Collect,
            &whose,
            Vec::new(),
            &device(12),
            at,
        ),
        at,
    );
    assert_eq!(state(&stranger), State::NotTaken as u64);
    assert_eq!(
        which(&stranger),
        post::why_not(almena_mailbox::asking::Not::NotThatDevice)
    );
}

#[test]
fn a_node_that_does_not_run_a_mailbox_has_no_such_route() {
    // **Not a refusal and not an error**: most nodes do not hold post, and the honest answer is
    // that there is nothing at that path. The gate is the node's own announcement, so what it
    // advertises and what it does cannot come apart.
    let mut node = Node::open(
        &Opening {
            which: Which::Development,
            beginning: Epoch::GENESIS,
            began: 1_800_000_000,
        },
        &[],
        &control(5),
        control(6),
    )
    .expect("nobody to join");
    let holds = device(11);
    let whose = an_account(&mut node, &control(9), &holds);
    let at = settled();

    assert_eq!(
        state(&post::deliver(
            &mut node,
            &whose.to_string(),
            &envelope("zTheIssuer", b"hello", 24),
            at
        )),
        State::NoSuchQuestion as u64
    );
    assert_eq!(
        state(&post::asked(
            &mut node,
            &asking(
                almena_mailbox::asking::Errand::Collect,
                &whose,
                Vec::new(),
                &holds,
                at
            ),
            at
        )),
        State::NoSuchQuestion as u64
    );
}

#[test]
fn a_frozen_account_takes_no_post_because_a_frozen_account_does_not_act() {
    // Freezing denies everything and concedes nothing (`SPECS.md §11.12`). Collecting is acting, so
    // it stops with everything else — and the post is still there when the account comes back.
    let mut node = a_mediator();
    let holds = device(11);
    let words = control(9);
    let whose = an_account(&mut node, &words, &holds);
    let at = settled();
    assert_eq!(
        state(&declaring(&mut node, &whose, &holds, at)),
        State::Taken as u64
    );
    assert_eq!(
        state(&post::deliver(
            &mut node,
            &whose.to_string(),
            &envelope("zTheIssuer", b"hello", 24),
            at
        )),
        State::Taken as u64
    );

    let head = node
        .chain_of(&whose, at)
        .answer
        .last()
        .expect("a chain")
        .hash
        .clone();
    let freeze = following(
        &After {
            whose: &whose,
            kind: Kind::HOLDER_FREEZE.number(),
            at,
            head,
        },
        BTreeMap::new(),
        &words,
    );
    node.submit(&freeze, at).expect("freezing does not wait");

    let collected = post::asked(
        &mut node,
        &asking(
            almena_mailbox::asking::Errand::Collect,
            &whose,
            Vec::new(),
            &holds,
            at,
        ),
        at,
    );
    assert_eq!(state(&collected), State::NotTaken as u64);
}
