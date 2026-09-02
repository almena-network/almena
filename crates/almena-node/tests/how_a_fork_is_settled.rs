//! An object whose chain split, and the one way out of it.
//!
//! **No node ever chooses a branch** (`SPECS.md §4.9`). Not the first it saw, not the one in more
//! roots, not the longer one — two honest nodes in different states with nobody having lied is the
//! one outcome this design cannot afford. So what this walks is the other half of that rule:
//! *unresolvable* cannot mean *blocked for ever*, and the tie is broken by somebody with the right
//! to sign on that object.
//!
//! What it proves in particular is the ugly case §4.9 names. Two rival `rotate` acts from one
//! predecessor — the owner's, and one made by whoever copied their twelve words — and then two
//! rival resolutions. The thief's is signed by the words alone, so it waits its seventy-two epochs
//! and any current device cancels it; the owner's is signed from a device and lands at once. **The
//! tie is broken by whoever has an apparatus in their hand**, which is exactly who should break it.

#![allow(clippy::expect_used, reason = "the whole of this file is a test")]

use std::collections::BTreeMap;

use almena_format::cbor::Value;
use almena_format::identifier::{Did, Name, Network};
use almena_format::operation::{Operation, Signed, create};
use almena_node::{Node, Opening};
use almena_store::chain::{Admitted, Answer, Reason, State};
use almena_store::genesis::Which;
use almena_store::kind::Kind;
use almena_store::resolution;
use almena_suite::{ed25519, p256};
use almena_time::Epoch;

/// After everything the words alone asked for has landed (`SPECS.md §11.12`).
fn settled() -> Epoch {
    Epoch::new(almena_time::deadline::CONTROL_KEY_WAIT.now() + 1)
}

fn words(seed: u8) -> ed25519::SigningKey {
    ed25519::SigningKey::from_secret([seed; 32])
}

fn device(seed: u8) -> p256::SigningKey {
    p256::SigningKey::from_secret([seed; 32]).expect("a valid scalar")
}

fn a_node() -> Node {
    Node::open(
        &Opening {
            which: Which::Development,
            beginning: Epoch::GENESIS,
            began: 1_800_000_000,
        },
        &[],
        &words(5),
        words(6),
    )
    .expect("nobody to join")
}

/// Sign an act with the words, which is what governs an account.
fn by_the_words(operation: &mut Operation, control: &ed25519::SigningKey) {
    let public = control.verifying_key().bytes();
    let signature = control.sign(&operation.signing_bytes());
    operation.signatures.push(Signed {
        by: operation.object.clone(),
        key: public.to_vec(),
        signature: signature.bytes(),
    });
}

/// Sign an act from one of the account's devices.
fn by_a_device(operation: &mut Operation, holds: &p256::SigningKey) {
    let over = operation.signing_bytes();
    operation.signatures.push(Signed {
        by: operation.object.clone(),
        key: holds.verifying_key().bytes().to_vec(),
        signature: holds.sign(&over).bytes(),
    });
}

/// An account with one device on it.
fn an_account(node: &mut Node, control: u8, holds: u8) -> (Did, Name) {
    let public = words(control).verifying_key().bytes();
    let mut created = create(
        Network::Development,
        Kind::HOLDER_CREATE.number(),
        1,
        Epoch::GENESIS,
        BTreeMap::from([(1, Value::Bytes(public.to_vec()))]),
    );
    by_the_words(&mut created, &words(control));
    let whose = created.object.clone();
    node.submit(&created, Epoch::GENESIS).expect("the account");

    let mut adding = following(
        &whose,
        &created.called(),
        Kind::HOLDER_ADD_DEVICE,
        Epoch::GENESIS,
        BTreeMap::from([(
            1,
            Value::Bytes(device(holds).verifying_key().bytes().to_vec()),
        )]),
    );
    by_the_words(&mut adding, &words(control));
    node.submit(&adding, Epoch::GENESIS).expect("the asking");
    (whose, adding.called())
}

/// One act on a chain that already exists.
fn following(
    whose: &Did,
    head: &Name,
    kind: Kind,
    at: Epoch,
    payload: BTreeMap<u64, Value>,
) -> Operation {
    Operation {
        object: whose.clone(),
        previous: Some(head.clone()),
        kind: kind.number(),
        version: 1,
        issued: at,
        payload,
        signatures: Vec::new(),
    }
}

/// A rotation of the control key, asked for by the words that govern the account today.
fn rotating(whose: &Did, head: &Name, to: u8) -> Operation {
    let mut asking = following(
        whose,
        head,
        Kind::HOLDER_ROTATE,
        settled(),
        BTreeMap::from([(1, Value::Bytes(words(to).verifying_key().bytes().to_vec()))]),
    );
    by_the_words(&mut asking, &words(1));
    asking
}

/// What the node says about that account.
fn answer(node: &Node, whose: &Did) -> Answer {
    node.resolve(whose.name(), settled()).answer
}

#[test]
fn a_fork_leaves_the_object_unresolvable_and_a_resolution_takes_it_out_again() {
    // **The whole of `SPECS.md §4.9` in one walk.** Two acts claim one predecessor; the node
    // declines to resolve rather than picking; and a device names the branch that survives.
    let mut node = a_node();
    let (whose, head) = an_account(&mut node, 1, 11);

    // The owner rotates their control key. So does whoever copied their words, from the same
    // predecessor — which is a fork made by two people each holding something that authorises it.
    let theirs = rotating(&whose, &head, 7);
    assert_eq!(
        node.submit(&theirs, settled()).expect("taken").answer,
        Admitted::Extended
    );

    let thiefs = rotating(&whose, &head, 9);
    assert_eq!(
        node.submit(&thiefs, settled()).expect("taken").answer,
        Admitted::Forked,
        "both are kept, and neither is chosen"
    );
    assert_eq!(
        answer(&node, &whose),
        Answer::CannotResolve(Reason::Forked),
        "and the node says so rather than serving one of them"
    );

    // The owner's device names the branch that survives. A device signs for itself, so it lands at
    // once — which is the asymmetry that settles this: the thief has no device and the owner does.
    let mut resolving = following(
        &whose,
        &theirs.called(),
        Kind::HOLDER_ADD_DEVICE,
        settled(),
        BTreeMap::from([(1, Value::Bytes(device(12).verifying_key().bytes().to_vec()))]),
    );
    resolution::declaring(&mut resolving, &theirs.called());
    by_a_device(&mut resolving, &device(11));
    assert_eq!(
        node.submit(&resolving, settled()).expect("taken").answer,
        Admitted::Resolves
    );

    // And the object resolves again, along the branch that was named.
    let Answer::Here(State::Holder(holder)) = answer(&node, &whose) else {
        panic!("it resolves again")
    };
    assert!(
        holder
            .devices
            .contains(device(12).verifying_key().bytes().as_slice()),
        "the act that carried the resolution did its own work, and a device's lands at once"
    );

    only_the_surviving_branch_is_waiting(&holder);
}

/// The second half of the test above, which is one function only because of its length.
///
/// **A rotation asked for by the words alone waits its seventy-two epochs** (`SPECS.md §11.12`), so
/// what the surviving branch establishes is visible in what is waiting — and the branch that lost
/// is not there at all.
fn only_the_surviving_branch_is_waiting(holder: &almena_store::chain::Holder) {
    let waiting: Vec<[u8; 32]> = holder
        .waiting
        .iter()
        .filter_map(|asking| match asking.does {
            almena_store::chain::Does::Rotate(key) => Some(key),
            _ => None,
        })
        .collect();
    assert_eq!(waiting, [words(7).verifying_key().bytes()]);

    let long_after = Epoch::new(settled().number() + 1_000);
    assert_eq!(
        holder.come_due(long_after).control,
        words(7).verifying_key().bytes(),
        "and when it lands it is the surviving branch's key and not the other one"
    );
}

#[test]
fn a_resolution_naming_a_branch_it_does_not_chain_from_is_not_one() {
    // An act whose own two halves disagree. There is no reading of it that is safe to pick, so it
    // is read as an ordinary act — which on a fork means it widens the fork rather than settling it.
    let mut node = a_node();
    let (whose, head) = an_account(&mut node, 1, 11);

    let mut one = following(
        &whose,
        &head,
        Kind::HOLDER_ADD_DEVICE,
        settled(),
        BTreeMap::from([(1, Value::Bytes(device(12).verifying_key().bytes().to_vec()))]),
    );
    by_a_device(&mut one, &device(11));
    node.submit(&one, settled()).expect("taken");

    let mut other = following(
        &whose,
        &head,
        Kind::HOLDER_ROTATE,
        settled(),
        BTreeMap::from([(1, Value::Bytes(words(9).verifying_key().bytes().to_vec()))]),
    );
    by_the_words(&mut other, &words(1));
    node.submit(&other, settled()).expect("taken");

    let mut crossed = following(
        &whose,
        &one.called(),
        Kind::HOLDER_ADD_DEVICE,
        settled(),
        BTreeMap::from([(1, Value::Bytes(device(13).verifying_key().bytes().to_vec()))]),
    );
    // Names one branch, chains from the other.
    resolution::declaring(&mut crossed, &other.called());
    by_a_device(&mut crossed, &device(11));
    assert_ne!(
        node.submit(&crossed, settled())
            .map(|answered| answered.answer),
        Ok(Admitted::Resolves),
        "an act that disagrees with itself settles nothing"
    );
    assert_eq!(answer(&node, &whose), Answer::CannotResolve(Reason::Forked));
}

#[test]
fn the_losing_branch_is_kept_and_left_without_effect() {
    // **Their authors can see that they landed nowhere** (`SPECS.md §4.9`), and repeat them if they
    // still want them. Deleting them would leave somebody unable to tell a refusal from a silence.
    let mut node = a_node();
    let (whose, head) = an_account(&mut node, 1, 11);

    let mut kept = following(
        &whose,
        &head,
        Kind::HOLDER_ADD_DEVICE,
        settled(),
        BTreeMap::from([(1, Value::Bytes(device(12).verifying_key().bytes().to_vec()))]),
    );
    by_a_device(&mut kept, &device(11));
    node.submit(&kept, settled()).expect("taken");

    let mut lost = following(
        &whose,
        &head,
        Kind::HOLDER_ROTATE,
        settled(),
        BTreeMap::from([(1, Value::Bytes(words(9).verifying_key().bytes().to_vec()))]),
    );
    by_the_words(&mut lost, &words(1));
    node.submit(&lost, settled()).expect("taken");

    let mut resolving = following(
        &whose,
        &kept.called(),
        Kind::HOLDER_ADD_DEVICE,
        settled(),
        BTreeMap::from([(1, Value::Bytes(device(13).verifying_key().bytes().to_vec()))]),
    );
    resolution::declaring(&mut resolving, &kept.called());
    by_a_device(&mut resolving, &device(11));
    node.submit(&resolving, settled()).expect("settled");

    assert!(
        node.act(&lost.called(), settled()).answer.is_some(),
        "the act that lost is still there to be read"
    );
    let Answer::Here(State::Holder(holder)) = answer(&node, &whose) else {
        panic!("it resolves again")
    };
    assert_eq!(
        holder.control,
        words(1).verifying_key().bytes(),
        "and what the losing branch asked for never happened"
    );
    assert!(
        holder.waiting.is_empty(),
        "not even as something waiting: the branch it was on has no effect at all"
    );
}

/// A directory of this test's own, removed when it is done with it.
struct Scratch(std::path::PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("almena-fork-{name}"));
        let _ = std::fs::remove_dir_all(&path);
        Self(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A node in `directory` holding an account whose fork was settled: the owner's rotation kept,
/// the thief's set aside, and a device added along the surviving branch.
fn a_settled_fork_in(directory: &std::path::Path) -> Did {
    let mut node = Node::open_in(
        directory,
        &Opening {
            which: Which::Development,
            beginning: Epoch::GENESIS,
            began: 1_800_000_000,
        },
        &[],
        &words(5),
        words(6),
    )
    .expect("nobody to join");
    let (whose, head) = an_account(&mut node, 1, 11);

    let theirs = rotating(&whose, &head, 7);
    node.submit(&theirs, settled()).expect("taken");
    let thiefs = rotating(&whose, &head, 9);
    assert_eq!(
        node.submit(&thiefs, settled()).expect("taken").answer,
        Admitted::Forked
    );

    let mut resolving = following(
        &whose,
        &theirs.called(),
        Kind::HOLDER_ADD_DEVICE,
        settled(),
        BTreeMap::from([(1, Value::Bytes(device(12).verifying_key().bytes().to_vec()))]),
    );
    resolution::declaring(&mut resolving, &theirs.called());
    by_a_device(&mut resolving, &device(11));
    assert_eq!(
        node.submit(&resolving, settled()).expect("taken").answer,
        Admitted::Resolves
    );
    assert!(matches!(
        answer(&node, &whose),
        Answer::Here(State::Holder(_))
    ));
    whose
}

#[test]
fn a_fork_settled_before_a_restart_stays_settled_after_it() {
    // **What a restart replays is admission, and admission alone does not settle a fork.** The
    // losing branch is put out of effect by replaying the branch the resolution named, which the
    // node did when the resolution arrived — and a restart that only re-admitted the acts would
    // bring the fork back on the very node that closed it, answering *forked again* to whoever
    // asked next. So the replay carries out the resolution exactly as the arrival did.
    let scratch = Scratch::new("settled-survives");
    let whose = a_settled_fork_in(&scratch.0);

    let back = Node::rejoin(&scratch.0, words(6)).expect("its own record");
    let Answer::Here(State::Holder(holder)) = answer(&back, &whose) else {
        panic!(
            "still settled after the restart: {:?}",
            answer(&back, &whose)
        )
    };
    assert!(
        holder
            .devices
            .contains(device(12).verifying_key().bytes().as_slice()),
        "along the branch that was named"
    );
    only_the_surviving_branch_is_waiting(&holder);
}
