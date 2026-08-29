//! The bytes every implementation of this format must agree on, byte for byte.
//!
//! `client` carries its own copy of the format, because the repositories share no code. Nothing
//! about the two being written by the same people makes them agree — what makes them agree is a
//! corpus of exact bytes each is held to, and this is the node's copy.
//!
//! **The twin belongs at `client/`, and the two must carry the same table.** It is not there yet,
//! because that repository is being rebuilt and there is nowhere to put it until it is.
//! Changing a byte here without changing it there is the failure this file exists to make loud,
//! and the day it happens the symptom is the worst one this platform has: **the same words open
//! two different accounts**, with an empty account and no error to explain it.
//!
//! # Why these three
//!
//! Nothing else in this format is allowed to be built on top of them until all three hold, and
//! they are not an arbitrary list:
//!
//! 1. **The same operation written two ways gives the same bytes and the same DID.** Without this,
//!    the promise that whoever holds the creation recomputes the name and checks it, without
//!    asking any node, is false.
//! 2. **An unknown critical field makes an operation uninterpretable.** The rule on which every
//!    reserved extension hole depends.
//! 3. **An `emitida` more than one epoch ahead is refused**, and refused against the *epoch*
//!    rather than the position, because two honest nodes must not disagree about validity.

// A corpus that cannot be read is a failing test, which is exactly what a panic here is.
#![allow(clippy::expect_used, reason = "the whole of this file is a test")]

use almena_format::cbor::Value;
use almena_format::entry::Entry;
use almena_format::field::{Field, Unintelligible, Vocabulary};
use almena_format::holes::{Carrier, HOLES, Protection, Ships};
use almena_format::identifier::{Did, Name, Network};
use almena_format::operation::{Operation, Signed, create};
use almena_time::{Clock, Epoch, Epochs};
use std::collections::BTreeMap;

/// The hex a run of bytes is written as.
fn hex_of(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// The one operation the corpus is built on: a holder creation, `tipo` 1, `version` 1, at the
/// genesis epoch, carrying one critical field and one that may be ignored.
fn payload() -> BTreeMap<u64, Value> {
    BTreeMap::from([
        (1, Value::Text("una clave de control".to_owned())),
        (2, Value::Uint(12)),
    ])
}

fn creation() -> Operation {
    create(Network::Development, 1, 1, Epoch::GENESIS, payload())
}

/// What the name is computed over: everything but `firmas` and `objeto`.
const NAMING: &str = "a502f603010401050006a20174756e6120636c61766520646520636f6e74726f6c020c";

/// What a signature covers: the same, with `objeto` restored.
const SIGNING: &str = "a601783e6469643a616c6d656e613a6465763a7a516d5061724677365077456f597235675745705459506165704d4d6d65515a48575358794b574e6267393353366402f603010401050006a20174756e6120636c61766520646520636f6e74726f6c020c";

/// And the name those bytes give the object.
const DID: &str = "did:almena:dev:zQmParFw6PwEoYr5gWEpTYPaepMMmeQZHWSXyKWNbg93S6d";

/// The log entry a node writes for it at position zero, with the operation signed.
///
/// **It moved when an act stopped being named over its signatures**, and the object's name did not
/// — which is the separation this corpus exists to hold, working. A name taken over the signatures
/// gave one act two of them, because an ECDSA signature has two valid forms for one message: an act
/// so named could be reprinted in the other form by anybody who saw it, and read as a second act on
/// the same chain.
const ENTRY: &str = "a6010002782f7a516d5667695456345736423176627a5a45377165615a7a775a4d53774733706e48736e4466394477656a6846744e03783e6469643a616c6d656e613a6465763a7a516d5061724677365077456f597235675745705459506165704d4d6d65515a48575358794b574e6267393353366404f605010601";

#[test]
fn the_operation_is_written_as_the_corpus_says() {
    let operation = creation();
    assert_eq!(hex_of(&operation.naming_bytes()), NAMING, "naming bytes");
    assert_eq!(hex_of(&operation.signing_bytes()), SIGNING, "signing bytes");
    assert_eq!(
        operation.object.to_string(),
        DID,
        "the name those bytes give it"
    );
}

#[test]
fn the_entry_is_written_as_the_corpus_says() {
    let mut operation = creation();
    operation.signatures.push(Signed {
        by: operation.object.clone(),
        key: vec![2; 33],
        signature: [0xab; 64],
    });
    assert_eq!(hex_of(&Entry::of(&operation, 0, None).to_bytes()), ENTRY);
}

#[test]
fn what_the_signatures_look_like_never_reaches_the_name() {
    // Proved the day the signature grew a field naming the key that made it: every entry hash in
    // this corpus moved, and not one object's name did. That is the separation working — a name
    // comes from the act, and how the act was signed is not part of the act.
    let mut operation = creation();
    let before = operation.object.to_string();

    operation.signatures.push(Signed {
        by: operation.object.clone(),
        key: vec![2; 33],
        signature: [0xab; 64],
    });
    operation.signatures.push(Signed {
        by: operation.object.clone(),
        key: vec![3; 33],
        signature: [0xcd; 64],
    });

    assert_eq!(operation.object.to_string(), before);
    assert_eq!(hex_of(&operation.naming_bytes()), NAMING);
    assert_eq!(hex_of(&operation.signing_bytes()), SIGNING);
}

#[test]
fn the_same_operation_built_two_ways_is_the_same_bytes_and_the_same_did() {
    // Criterion 1. "Two ways" is not a hypothetical: one program builds the payload as it reads
    // the user's answers, another as it walks a schema, and the maps come out in opposite orders.
    // If that changed a byte, it would change the name, and the two would disagree about who the
    // person is while both believing they had followed the format to the letter.
    let backwards = {
        let mut fields = BTreeMap::new();
        fields.insert(2, Value::Uint(12));
        fields.insert(1, Value::Text("una clave de control".to_owned()));
        fields
    };
    let other_way = create(Network::Development, 1, 1, Epoch::GENESIS, backwards);

    assert_eq!(other_way.naming_bytes(), creation().naming_bytes());
    assert_eq!(other_way.object.to_string(), DID);
}

#[test]
fn the_name_is_recomputable_by_whoever_holds_the_creation() {
    // The self-certifying name, checked the way a stranger would check it: hash the bytes, compare
    // with what the operation calls itself. No node is asked anything.
    let operation = creation();
    assert_eq!(
        Did::new(Network::Development, Name::of(&operation.naming_bytes())).to_string(),
        DID
    );
    assert!(operation.names_itself());
}

#[test]
fn an_unknown_critical_field_makes_it_uninterpretable() {
    // Criterion 2. Field 3 is odd, so a reader that does not know it may not claim to have
    // applied this operation — the certification-with-a-scope read as one without limits.
    let mut extended = creation();
    extended
        .payload
        .insert(3, Value::Text("un alcance".to_owned()));

    let known = [Field::new(1), Field::new(2)];
    assert_eq!(
        extended.understood(Vocabulary::of(&known)),
        Err(Unintelligible::Field(Field::new(3)))
    );
}

#[test]
fn an_unknown_field_that_may_be_ignored_does_not() {
    // The other half, and the one that makes the reserved holes usable at all: field 4 is even,
    // so an old reader passes over it and applies the operation.
    let mut extended = creation();
    extended
        .payload
        .insert(4, Value::Text("algo nuevo".to_owned()));
    let known = [Field::new(1), Field::new(2)];
    assert_eq!(extended.understood(Vocabulary::of(&known)), Ok(()));

    // And it is still a different object, because the field is in the bytes either way.
    assert_ne!(extended.name(), creation().name());
}

#[test]
fn a_value_outside_a_closed_vocabulary_is_refused() {
    // Criterion 2, second door. Three of the six reserved holes ship a field on day one and grow by
    // *value* — a credential's proof type, the issuer's identification method, and a proposal's
    // `método`. Every reader knows those fields, so their parity never fires, and without this an
    // old reader would take the value it has never seen for the one it has: `ciego` counted as
    // `abierto`, a vote tallied by someone who did not understand it.
    let method = Field::new(1);
    const OPEN: Value = Value::Uint(0);
    const BLIND: Value = Value::Uint(1);

    let mut proposal = creation();
    proposal.payload.insert(method.number(), BLIND);

    let fields = [method];
    let before = [(method, &[OPEN][..])];
    let after = [(method, &[OPEN, BLIND][..])];

    assert_eq!(
        proposal.understood(Vocabulary::with_closed(&fields, &before)),
        Err(Unintelligible::Value(method)),
        "a reader that has not been updated refuses"
    );
    assert_eq!(
        proposal.understood(Vocabulary::with_closed(&fields, &after)),
        Ok(()),
        "one that has, applies it — which is what makes it an addition and not a migration"
    );
}

#[test]
fn the_table_of_holes_says_what_protects_each_one() {
    // All six were once thought to depend on the criticality mark. They do not: only a hole that
    // lives in a payload can be protected by a field number at all, and this is the corpus row
    // that keeps the table honest about which mechanism each one actually uses.
    assert_eq!(HOLES.len(), 6);
    for hole in HOLES {
        if hole.protection.contains(&Protection::Criticality) {
            assert_eq!(hole.carrier, Carrier::Payload, "{}", hole.name);
        }
        if hole.ships == Ships::Now {
            assert!(
                hole.protection.contains(&Protection::ClosedVocabulary),
                "{} ships now, so it grows by value",
                hole.name
            );
        }
    }
}

#[test]
fn an_emitida_more_than_one_epoch_ahead_is_refused() {
    // Criterion 3. The tolerance of one epoch is not generosity: without it a node whose clock is
    // five minutes slow would refuse legitimate operations for those five minutes of every hour,
    // right at the boundary — and two honest nodes would disagree again, which is the outcome the
    // rule existed to prevent.
    let current = Epoch::GENESIS.plus(Epochs(10)).expect("no overflow");
    let cases = [
        (Epoch::GENESIS, true, "the past is always acceptable"),
        (current, true, "the present"),
        (
            current.plus(Epochs(1)).expect("no overflow"),
            true,
            "clock drift, absorbed",
        ),
        (
            current.plus(Epochs(2)).expect("no overflow"),
            false,
            "the future is not",
        ),
        (
            current.plus(Epochs(9_999)).expect("no overflow"),
            false,
            "nor is it far away",
        ),
    ];
    for (declared, accepted, why) in cases {
        assert_eq!(Clock::accepts(declared, current), accepted, "{why}");
    }
}

#[test]
fn every_form_in_the_corpus_is_canonical() {
    // The corpus cannot contain bytes the profile would refuse: they would be a contract to
    // produce something invalid.
    for hex in [NAMING, SIGNING, ENTRY] {
        let bytes: Vec<u8> = (0..hex.len())
            .step_by(2)
            .map(|at| u8::from_str_radix(&hex[at..at + 2], 16).expect("the corpus is hexadecimal"))
            .collect();
        assert_eq!(almena_cbor::canonical(&bytes), Ok(()), "{hex}");
    }
}
