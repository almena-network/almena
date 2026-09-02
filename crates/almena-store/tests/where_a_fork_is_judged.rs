//! Two honest nodes, the same two acts, and the order they arrived in.
//!
//! **The one thing that must never decide anything.** A second act claiming a predecessor that
//! already has a successor is judged against the account as it stood *at that predecessor* —
//! rebuilt along the branch the act is actually on — and never against the head, which is only
//! whichever branch this node happened to hear of first. Judged at the head, a device added after
//! the fork point signs successfully on the node that heard about it first and is a stranger on
//! the node that did not, and two correct implementations reach two answers from one pair of acts.
//!
//! What this walks is that property, and its edge: a node that cannot rebuild the branch says so
//! recoverably rather than guessing.

#![allow(clippy::expect_used, reason = "the whole of this file is a test")]

use std::collections::BTreeMap;

use almena_format::cbor::Value;
use almena_format::identifier::{Did, Name, Network};
use almena_format::operation::{Operation, Signed, create};
use almena_store::chain::{Admitted, Answer, Objects, Reason, Refused, State};
use almena_store::kind::Kind;
use almena_suite::{ed25519, p256};
use almena_time::Epoch;

/// After everything the words alone asked for has landed.
fn settled() -> Epoch {
    Epoch::new(almena_time::deadline::CONTROL_KEY_WAIT.now() + 1)
}

/// A little later still, for acts that follow the settled ones.
fn later(by: u64) -> Epoch {
    Epoch::new(settled().number() + by)
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

/// An unsigned act adding a device, chained from `head`.
fn add_device(whose: &Did, head: &Name, holds: u8, at: Epoch) -> Operation {
    Operation {
        object: whose.clone(),
        previous: Some(head.clone()),
        kind: Kind::HOLDER_ADD_DEVICE.number(),
        version: 1,
        issued: at,
        payload: BTreeMap::from([(
            1,
            Value::Bytes(device(holds).verifying_key().bytes().to_vec()),
        )]),
        signatures: Vec::new(),
    }
}

/// An account whose words put device 11 on it at the genesis, and the two acts that did so.
///
/// Everything a test forks from is the second of them: the point where the account has exactly
/// one device, which is what makes *added after the fork point* a thing that can be said.
fn an_account() -> (Vec<Operation>, Did, Name) {
    let created = creation(1);
    let whose = created.object.clone();
    let mut first = add_device(&whose, &created.called(), 11, Epoch::GENESIS);
    by_the_words(&mut first, &words(1));
    let fork_point = first.called();
    (vec![created, first], whose, fork_point)
}

fn holding(objects: &mut Objects, acts: &[Operation]) {
    for act in acts {
        objects.admit(act, settled()).expect("taken");
    }
}

fn devices_on(objects: &Objects, whose: &Did) -> Answer {
    match objects.resolve(whose.name()) {
        Answer::Here(State::Holder(holder)) => {
            Answer::Here(State::Holder(holder.come_due(later(10))))
        }
        other => other,
    }
}

fn how_many_devices(objects: &Objects, whose: &Did) -> usize {
    match devices_on(objects, whose) {
        Answer::Here(State::Holder(holder)) => holder.devices.len(),
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_device_added_after_the_fork_point_is_a_stranger_at_it_whatever_arrived_first() {
    // X: device 11 adds device 12, from the fork point. F: device 12 signs an act from the same
    // fork point — where device 12 did not exist yet. F is not entitled, on any node, in any order.
    let (opening, whose, fork_point) = an_account();
    let mut x = add_device(&whose, &fork_point, 12, settled());
    by_a_device(&mut x, &device(11));
    let mut f = add_device(&whose, &fork_point, 13, settled());
    by_a_device(&mut f, &device(12));

    let mut heard_x_first = Objects::new();
    holding(&mut heard_x_first, &opening);
    assert_eq!(heard_x_first.admit(&x, settled()), Ok(Admitted::Extended));
    assert_eq!(
        heard_x_first.admit(&f, settled()),
        Err(Refused::NotAuthorised),
        "judged at the fork point, device 12 is nobody — even though the head knows it"
    );

    let mut heard_f_first = Objects::new();
    holding(&mut heard_f_first, &opening);
    assert_eq!(
        heard_f_first.admit(&f, settled()),
        Err(Refused::NotAuthorised)
    );
    assert_eq!(heard_f_first.admit(&x, settled()), Ok(Admitted::Extended));

    assert_eq!(
        devices_on(&heard_x_first, &whose),
        devices_on(&heard_f_first, &whose),
        "two honest nodes, one answer"
    );
    assert_eq!(how_many_devices(&heard_x_first, &whose), 2);
}

#[test]
fn a_signer_entitled_at_the_fork_point_splits_the_object_whatever_arrived_first() {
    // The rule the one above must not have weakened: two acts somebody had the right to sign
    // still make the object one nobody resolves, in either order.
    let (opening, whose, fork_point) = an_account();
    let mut x = add_device(&whose, &fork_point, 12, settled());
    by_a_device(&mut x, &device(11));
    let mut y = add_device(&whose, &fork_point, 13, settled());
    by_a_device(&mut y, &device(11));

    for (first, second) in [(&x, &y), (&y, &x)] {
        let mut objects = Objects::new();
        holding(&mut objects, &opening);
        assert_eq!(objects.admit(first, settled()), Ok(Admitted::Extended));
        assert_eq!(objects.admit(second, settled()), Ok(Admitted::Forked));
        assert_eq!(
            objects.resolve(whose.name()),
            Answer::CannotResolve(Reason::Forked)
        );
    }
}

#[test]
fn an_act_dated_after_its_predecessor_but_before_the_head_is_not_rewinding_anything() {
    // The date is checked against the act it follows. Checked against the head it would refuse
    // on the node where the other branch arrived first and take on the node where it did not.
    let (opening, whose, fork_point) = an_account();
    let mut x = add_device(&whose, &fork_point, 12, later(5));
    by_a_device(&mut x, &device(11));
    let mut f = add_device(&whose, &fork_point, 13, later(1));
    by_a_device(&mut f, &device(11));

    for (first, second) in [(&x, &f), (&f, &x)] {
        let mut objects = Objects::new();
        holding(&mut objects, &opening);
        assert_eq!(objects.admit(first, later(10)), Ok(Admitted::Extended));
        assert_eq!(
            objects.admit(second, later(10)),
            Ok(Admitted::Forked),
            "dated after the fork point is all that is asked"
        );
    }
}

/// A long branch from the fork point: device 11 adding one device after another.
///
/// Longer than the states a chain keeps beside it, so that a fork from its start has to be
/// rebuilt from the record rather than read off the cache.
fn a_long_branch(whose: &Did, from: &Name) -> Vec<Operation> {
    let mut branch = Vec::new();
    let mut head = from.clone();
    for holds in 20..60 {
        let mut adding = add_device(whose, &head, holds, settled());
        by_a_device(&mut adding, &device(11));
        head = adding.called();
        branch.push(adding);
    }
    branch
}

fn by_name(acts: &[Operation]) -> BTreeMap<Name, Operation> {
    acts.iter().map(|act| (act.called(), act.clone())).collect()
}

#[test]
fn a_fork_further_back_than_the_kept_states_is_rebuilt_from_the_record() {
    let (opening, whose, fork_point) = an_account();
    let branch = a_long_branch(&whose, &fork_point);
    let mut objects = Objects::new();
    holding(&mut objects, &opening);
    holding(&mut objects, &branch);

    // Entitled at the fork point, and so a fork; signed by a device that only exists further up
    // the branch, and so a stranger. Neither is decidable from the head.
    let mut entitled = add_device(&whose, &fork_point, 70, settled());
    by_a_device(&mut entitled, &device(11));
    let mut stranger = add_device(&whose, &fork_point, 71, settled());
    by_a_device(&mut stranger, &device(25));

    assert_eq!(
        objects.admit(&entitled, settled()),
        Err(Refused::BranchNotHeld),
        "without the record to hand, nothing is guessed"
    );

    let mut held = by_name(&opening);
    held.extend(by_name(&branch));
    let record = |name: &Name| held.get(name).cloned();
    assert_eq!(
        objects.admit_from(&stranger, settled(), &record),
        Err(Refused::NotAuthorised)
    );
    assert_eq!(
        objects.admit_from(&entitled, settled(), &record),
        Ok(Admitted::Forked)
    );
    assert_eq!(
        objects.resolve(whose.name()),
        Answer::CannotResolve(Reason::Forked)
    );
}

#[test]
fn a_branch_this_node_let_go_of_is_refused_until_it_is_fetched() {
    // Recoverable, like an act the disk would not take: nothing about the act is wrong, and
    // handing it over again once the branch is here is what mends it.
    let (opening, whose, fork_point) = an_account();
    let branch = a_long_branch(&whose, &fork_point);
    let mut objects = Objects::new();
    holding(&mut objects, &opening);
    holding(&mut objects, &branch);

    let mut forking = add_device(&whose, &fork_point, 70, settled());
    by_a_device(&mut forking, &device(11));

    // The creation was dealt elsewhere and let go of here.
    let mut short = by_name(&branch);
    short.insert(opening[1].called(), opening[1].clone());
    let without_the_creation = |name: &Name| short.get(name).cloned();
    assert_eq!(
        objects.admit_from(&forking, settled(), &without_the_creation),
        Err(Refused::BranchNotHeld)
    );

    let mut whole = short.clone();
    whole.insert(opening[0].called(), opening[0].clone());
    let fetched = |name: &Name| whole.get(name).cloned();
    assert_eq!(
        objects.admit_from(&forking, settled(), &fetched),
        Ok(Admitted::Forked),
        "delivered again after fetching, it is judged"
    );
}

#[test]
fn a_fork_near_the_head_needs_no_record_at_all() {
    // The common case, judged off what the chain keeps beside it.
    let (opening, whose, fork_point) = an_account();
    let mut objects = Objects::new();
    holding(&mut objects, &opening);
    let mut head = fork_point.clone();
    for holds in 20..25 {
        let mut adding = add_device(&whose, &head, holds, settled());
        by_a_device(&mut adding, &device(11));
        head = adding.called();
        assert_eq!(objects.admit(&adding, settled()), Ok(Admitted::Extended));
    }

    let mut forking = add_device(&whose, &fork_point, 70, settled());
    by_a_device(&mut forking, &device(11));
    assert_eq!(objects.admit(&forking, settled()), Ok(Admitted::Forked));
}
