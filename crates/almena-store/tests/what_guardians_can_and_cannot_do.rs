//! Losing every device, and the way back that does not hand anybody the account.
//!
//! # The asymmetry is the whole design
//!
//! `SPECS.md §11.4`, which is `SPECS.md §1.8` said again: **what takes trust away is accepted
//! quickly; what grants it waits.** A quorum of guardians freezes at once, because freezing denies
//! everything and concedes nothing — two of them colluding gain a nuisance and not an identity.
//! Only the holder starts a recovery, and it waits where any device still in their hands says no.
//!
//! # And the record never says who they are
//!
//! A public list of the people who can freeze somebody is a list of the people to go after. What
//! the chain carries is a Merkle root; a guardian who acts shows their own leaf and the path to it,
//! reveals themselves — which signing did anyway — and reveals nothing about the others.

#![allow(clippy::expect_used, reason = "the whole of this file is a test")]

use std::collections::BTreeMap;

use almena_format::cbor::Value;
use almena_format::identifier::{Did, Name, Network};
use almena_format::operation::{Operation, Signed, create};
use almena_store::chain::{Admitted, Answer, Objects, Refused, State};
use almena_store::guardian::{self, Proof, SALT_WIDTH, carried, commit, leaf};
use almena_store::kind::Kind;
use almena_store::tree::Tree;
use almena_suite::{ed25519, p256};
use almena_time::{Epoch, Epochs};

/// When everything below happens.
fn now() -> Epoch {
    Epoch::new(100)
}

/// After everything the control key alone asked for has landed.
fn settled() -> Epoch {
    now()
        .plus(Epochs(almena_time::deadline::CONTROL_KEY_WAIT.now() + 1))
        .expect("no overflow")
}

fn words(seed: u8) -> ed25519::SigningKey {
    ed25519::SigningKey::from_secret([seed; 32])
}

fn device(seed: u8) -> p256::SigningKey {
    p256::SigningKey::from_secret([seed; 32]).expect("a valid scalar")
}

/// One person's account, with one device on it and operative.
fn a_person(objects: &mut Objects, control: u8, holds: u8) -> Did {
    let key = words(control);
    let public = key.verifying_key().bytes();
    let mut created = create(
        Network::Development,
        Kind::HOLDER_CREATE.number(),
        1,
        Epoch::GENESIS,
        BTreeMap::from([(1, Value::Bytes(public.to_vec()))]),
    );
    let whose = created.object.clone();
    sign_with_words(&mut created, &whose, &key);
    objects
        .admit(&created, Epoch::GENESIS)
        .expect("the account");

    let head = objects.head(whose.name()).expect("a head").clone();
    let mut adding = act(
        &whose,
        &head,
        Kind::HOLDER_ADD_DEVICE,
        Epoch::GENESIS,
        &[(
            1,
            Value::Bytes(device(holds).verifying_key().bytes().to_vec()),
        )],
    );
    sign_with_words(&mut adding, &whose, &key);
    objects.admit(&adding, Epoch::GENESIS).expect("the asking");
    whose
}

/// One act on a chain that already exists.
fn act(object: &Did, head: &Name, kind: Kind, at: Epoch, payload: &[(u64, Value)]) -> Operation {
    Operation {
        object: object.clone(),
        previous: Some(head.clone()),
        kind: kind.number(),
        version: 1,
        issued: at,
        payload: payload.iter().cloned().collect(),
        signatures: Vec::new(),
    }
}

/// Sign as the account itself, with the key the words give.
fn sign_with_words(operation: &mut Operation, whose: &Did, key: &ed25519::SigningKey) {
    let signature = key.sign(&operation.signing_bytes());
    operation.signatures.push(Signed {
        by: whose.clone(),
        key: key.verifying_key().bytes().to_vec(),
        signature: signature.bytes(),
    });
}

/// Sign as somebody, from one of their devices.
fn sign_as(operation: &mut Operation, who: &Did, holds: u8) {
    let signature = device(holds).sign(&operation.signing_bytes());
    operation.signatures.push(Signed {
        by: who.clone(),
        key: device(holds).verifying_key().bytes().to_vec(),
        signature: signature.bytes(),
    });
}

/// A salt, as a wallet would draw one.
fn salt(seed: u8) -> Vec<u8> {
    vec![seed; SALT_WIDTH]
}

/// The record, an account, its guardians, and the proofs each of them holds.
struct Set {
    objects: Objects,
    /// The account being looked after.
    whose: Did,
    /// The device it started with.
    holds: u8,
    /// The guardians, each with the salt their leaf was made with and the device they sign from.
    guardians: Vec<(Did, Vec<u8>, u8)>,
}

impl Set {
    /// The proof one guardian holds.
    fn proof(&self, which: usize) -> Proof {
        let mut tree = Tree::new();
        for (who, salt, _) in &self.guardians {
            tree.append(&leaf(who, salt));
        }
        Proof {
            guardian: self.guardians[which].0.clone(),
            salt: self.guardians[which].1.clone(),
            at: which as u64,
            path: tree.inclusion(which).expect("a path"),
        }
    }

    /// The account's head.
    fn head(&self) -> Name {
        self.objects
            .head(self.whose.name())
            .expect("a head")
            .clone()
    }

    /// The account as it stands at that moment.
    fn holder(&self, at: Epoch) -> almena_store::chain::Holder {
        match self.objects.resolve(self.whose.name()) {
            Answer::Here(State::Holder(held)) => held.come_due(at),
            other => panic!("{other:?}"),
        }
    }
}

/// An account with three guardians, two of whom are enough.
fn a_set() -> Set {
    let mut objects = Objects::new();
    let whose = a_person(&mut objects, 1, 11);
    let guardians: Vec<(Did, Vec<u8>, u8)> = (0..3)
        .map(|which| {
            let who = a_person(&mut objects, 20 + which, 40 + which);
            (who, salt(60 + which), 40 + which)
        })
        .collect();

    let listed: Vec<(Did, Vec<u8>)> = guardians
        .iter()
        .map(|(who, salt, _)| (who.clone(), salt.clone()))
        .collect();
    let head = objects.head(whose.name()).expect("a head").clone();
    let mut naming = act(
        &whose,
        &head,
        Kind::HOLDER_SET_GUARDIANS,
        now(),
        &[
            (
                guardian::field::COMMITMENT,
                Value::Bytes(commit(&listed).bytes().to_vec()),
            ),
            (guardian::field::HOW_MANY, Value::Uint(3)),
            (guardian::field::ENOUGH, Value::Uint(2)),
        ],
    );
    // Named from the device, which is immediate: holding it back would make the safe direction the
    // expensive one, which is how somebody ends up with no guardians at all.
    sign_as(&mut naming, &whose, 11);
    objects.admit(&naming, now()).expect("named");

    Set {
        objects,
        whose,
        holds: 11,
        guardians,
    }
}

/// The freeze a quorum of guardians signs.
fn freezing(set: &Set, which: &[usize], at: Epoch) -> Operation {
    let proofs: Vec<Value> = which.iter().map(|one| carried(&set.proof(*one))).collect();
    let mut freeze = act(
        &set.whose,
        &set.head(),
        Kind::HOLDER_FREEZE,
        at,
        &[(guardian::field::PROOFS, Value::Array(proofs))],
    );
    for one in which {
        let (who, _, holds) = &set.guardians[*one];
        sign_as(&mut freeze, who, *holds);
    }
    freeze
}

/// The recovery a holder asks for, with a new key and a new device.
fn recovering(set: &Set, which: &[usize], control: u8, holds: u8, at: Epoch) -> Operation {
    let proofs: Vec<Value> = which.iter().map(|one| carried(&set.proof(*one))).collect();
    let mut recover = act(
        &set.whose,
        &set.head(),
        Kind::HOLDER_RECOVER,
        at,
        &[
            (
                1,
                Value::Bytes(words(control).verifying_key().bytes().to_vec()),
            ),
            (
                guardian::field::DEVICE,
                Value::Bytes(device(holds).verifying_key().bytes().to_vec()),
            ),
            (guardian::field::PROOFS, Value::Array(proofs)),
        ],
    );
    // **Signed by the key it establishes**, which is what says whoever asked holds it.
    sign_with_words(&mut recover, &set.whose, &words(control));
    for one in which {
        let (who, _, holds) = &set.guardians[*one];
        sign_as(&mut recover, who, *holds);
    }
    recover
}

#[test]
fn the_record_never_says_who_the_guardians_are() {
    // **A public list of the people who can freeze somebody is a list of the people to go after.**
    // What is in the chain is a root and two numbers.
    let set = a_set();
    let held = set.holder(now()).guardians.expect("named");
    assert_eq!(held.how_many, 3);
    assert_eq!(held.enough, 2);

    for (who, _, _) in &set.guardians {
        assert!(
            !format!("{:?}", set.holder(now())).contains(who.name().as_str()),
            "and none of them is anywhere in what the account says"
        );
    }
}

#[test]
fn a_quorum_of_guardians_freezes_at_once_and_one_of_them_does_not() {
    // **`SPECS.md §1.8` said again.** Freezing denies everything and concedes nothing, so two
    // colluding gain a nuisance and never an identity — and it lands at once, because the phone in
    // somebody else's pocket has to go inert now and not in three days.
    let mut set = a_set();

    let alone = freezing(&set, &[0], now());
    assert_eq!(
        set.objects.admit(&alone, now()),
        Err(Refused::NotAuthorised),
        "one guardian is not a quorum"
    );

    let quorum = freezing(&set, &[0, 2], now());
    assert_eq!(set.objects.admit(&quorum, now()), Ok(Admitted::Extended));
    assert!(set.holder(now()).frozen, "and it lands at once");
}

#[test]
fn a_guardian_who_did_not_sign_counts_as_nobody() {
    // A proof is a claim to be a guardian; what makes it an act of theirs is the signature, checked
    // against a key their own chain authorises. Two proofs and one signature is one guardian.
    let mut set = a_set();
    let proofs: Vec<Value> = [0, 1].iter().map(|one| carried(&set.proof(*one))).collect();
    let mut freeze = act(
        &set.whose,
        &set.head(),
        Kind::HOLDER_FREEZE,
        now(),
        &[(guardian::field::PROOFS, Value::Array(proofs))],
    );
    let (who, _, holds) = &set.guardians[0];
    sign_as(&mut freeze, who, *holds);

    assert_eq!(
        set.objects.admit(&freeze, now()),
        Err(Refused::NotAuthorised)
    );
}

#[test]
fn somebody_who_is_not_a_guardian_proves_nothing_by_signing() {
    // The commitment is what says who is one, and a stranger's leaf is not under it.
    let mut set = a_set();
    let stranger = a_person(&mut set.objects, 90, 91);

    let mut freeze = act(
        &set.whose,
        &set.head(),
        Kind::HOLDER_FREEZE,
        now(),
        &[(
            guardian::field::PROOFS,
            Value::Array(vec![
                carried(&set.proof(0)),
                carried(&Proof {
                    guardian: stranger.clone(),
                    ..set.proof(1)
                }),
            ]),
        )],
    );
    let (who, _, holds) = &set.guardians[0];
    sign_as(&mut freeze, who, *holds);
    sign_as(&mut freeze, &stranger, 91);

    assert_eq!(
        set.objects.admit(&freeze, now()),
        Err(Refused::NotAuthorised)
    );
}

#[test]
fn a_recovery_waits_and_any_device_still_in_the_holders_hands_stops_it() {
    // **The defence against collusion and against the guardian talked into it over the phone.**
    // Reaching the quorum is not executing: the window is what lets somebody who still has a device
    // say no.
    let mut set = a_set();
    let asking = recovering(&set, &[0, 1], 77, 78, now());
    assert_eq!(set.objects.admit(&asking, now()), Ok(Admitted::Extended));

    let waiting = set.holder(now()).waiting;
    assert_eq!(waiting.len(), 1, "it is asked for, not done");
    assert_eq!(set.holder(now()).devices.len(), 1, "and nothing has moved");

    // The holder still has their phone, and says no.
    let mut cancel = act(
        &set.whose,
        &set.head(),
        Kind::HOLDER_CANCEL,
        now(),
        &[(1, Value::Text(waiting[0].act.as_str().to_owned()))],
    );
    sign_as(&mut cancel, &set.whose, set.holds);
    assert_eq!(set.objects.admit(&cancel, now()), Ok(Admitted::Extended));
    assert!(set.holder(settled()).waiting.is_empty());
    assert_ne!(
        set.holder(settled()).control,
        words(77).verifying_key().bytes(),
        "and the account did not change hands"
    );
}

#[test]
fn a_recovery_that_nobody_stopped_leaves_the_account_operating() {
    // **Atomic, and that is the whole of it** (`SPECS.md §11.4`): a new key governing, nothing of
    // the old devices left, and the one the asker brought on it. It comes out operative, not empty.
    let mut set = a_set();
    let asking = recovering(&set, &[0, 1], 77, 78, now());
    set.objects.admit(&asking, now()).expect("asked");

    let after = set.holder(settled());
    assert_eq!(after.control, words(77).verifying_key().bytes());
    assert_eq!(
        after.devices,
        std::collections::BTreeSet::from([device(78).verifying_key().bytes().to_vec()]),
        "the old device is gone and the new one is on"
    );
    assert!(after.waiting.is_empty());
}

#[test]
fn a_recovered_account_can_still_say_what_it_is_and_cite_what_made_it_so() {
    // **A summary of a recovered account is checkable and not *cannot say*.** A recovery replaces
    // the control key, empties the devices and enrols the one that asked — three things at once,
    // and a summary citing an earlier `add_device` would stand while describing devices that were
    // wiped. So the recovery is what both parts of the claim cite, and whoever checks the summary
    // fetches that act and replays it.
    use almena_store::checkpoint::{Governs, Stated};

    let mut set = a_set();
    let asking = recovering(&set, &[0, 1], 77, 78, now());
    let recovered = asking.called();
    set.objects.admit(&asking, now()).expect("asked");

    let standing = set
        .objects
        .standing(set.whose.name(), settled())
        .expect("it resolves, and it has something to claim");

    let control = standing
        .claims
        .iter()
        .find(|claim| claim.about == Governs::Control)
        .expect("the key that governs it");
    assert_eq!(
        control.stated,
        Stated::Key(words(77).verifying_key().bytes().to_vec())
    );
    assert_eq!(
        control.set_by, recovered,
        "and it cites the recovery, which is the act that put it there"
    );

    let devices = standing
        .claims
        .iter()
        .find(|claim| claim.about == Governs::Devices)
        .expect("what operates it");
    assert_eq!(
        devices.stated,
        Stated::Keys(std::collections::BTreeSet::from([device(78)
            .verifying_key()
            .bytes()
            .to_vec()]))
    );
    assert_eq!(
        devices.set_by, recovered,
        "the same act, because emptying the set and enrolling one device are the same act"
    );
}

#[test]
fn a_frozen_account_is_exactly_the_one_a_recovery_is_for() {
    // The guardians froze it when the phone was lost. Making the holder thaw it first would be
    // asking them to concede the account back before they can take it — and thawing needs the words
    // they no longer have.
    let mut set = a_set();
    let quorum = freezing(&set, &[0, 1], now());
    set.objects.admit(&quorum, now()).expect("frozen");
    assert!(set.holder(now()).frozen);

    let asking = recovering(&set, &[0, 1], 77, 78, now());
    assert_eq!(set.objects.admit(&asking, now()), Ok(Admitted::Extended));

    let after = set.holder(settled());
    assert!(
        !after.frozen,
        "and it comes out thawed, or the way back would be to ask the guardians again"
    );
    assert_eq!(after.control, words(77).verifying_key().bytes());
}

#[test]
fn guardians_alone_cannot_take_an_account() {
    // **Only the holder starts it** (`SPECS.md §11.4`). If a guardian could, two colluding would
    // rotate somebody's identity to themselves. The act carries the new control key and is signed
    // by it, so a quorum with no such signature is a quorum asking for nothing.
    let mut set = a_set();
    let proofs: Vec<Value> = [0, 1].iter().map(|one| carried(&set.proof(*one))).collect();
    let mut theirs = act(
        &set.whose,
        &set.head(),
        Kind::HOLDER_RECOVER,
        now(),
        &[
            (1, Value::Bytes(words(77).verifying_key().bytes().to_vec())),
            (
                guardian::field::DEVICE,
                Value::Bytes(device(78).verifying_key().bytes().to_vec()),
            ),
            (guardian::field::PROOFS, Value::Array(proofs)),
        ],
    );
    for one in [0, 1] {
        let (who, _, holds) = &set.guardians[one];
        sign_as(&mut theirs, who, *holds);
    }
    assert_eq!(
        set.objects.admit(&theirs, now()),
        Err(Refused::SignatureDoesNotCheck),
        "the key it establishes did not sign, so nobody proved they hold it"
    );
}

#[test]
fn an_account_with_no_guardians_is_not_one_anybody_can_freeze_this_way() {
    // Nought guardians and a proof of nothing would otherwise be a stranger's freeze.
    let mut objects = Objects::new();
    let whose = a_person(&mut objects, 1, 11);
    let stranger = a_person(&mut objects, 90, 91);
    let head = objects.head(whose.name()).expect("a head").clone();

    let mut freeze = act(
        &whose,
        &head,
        Kind::HOLDER_FREEZE,
        now(),
        &[(
            guardian::field::PROOFS,
            Value::Array(vec![carried(&Proof {
                guardian: stranger.clone(),
                salt: salt(1),
                at: 0,
                path: almena_store::tree::Path::of(Vec::new()),
            })]),
        )],
    );
    sign_as(&mut freeze, &stranger, 91);
    assert_eq!(objects.admit(&freeze, now()), Err(Refused::NotAuthorised));
}
