//! Settling an object whose chain has split.
//!
//! # Nothing here lets a node choose a branch
//!
//! A forked object is one this node **declines to resolve** (`SPECS.md §4.9`). It does not pick the
//! first branch it saw, nor the one in more roots, nor the longer one: two honest nodes in
//! different states with nobody having lied is the one outcome this design cannot afford. So the
//! tie is always broken by **somebody with the right to sign on that object**, and this module is
//! what such a person's act looks like.
//!
//! *Unresolvable* still cannot mean *blocked for ever*, which is what makes the act necessary.
//!
//! # It rides on any act, because it is a fact about the act and not a kind of act
//!
//! A resolution is not a new operation. It is a **field any operation may carry**, exactly as a
//! summary is (`SPECS.md §4.6`, `§4.9`) — the same shape and for the same reason: naming the branch
//! that survives is the same idea whatever the act happens to do, and writing it as an exception
//! per kind would make it as many coincidences as there are kinds. So it sits above the line where
//! a number means one thing everywhere.
//!
//! **And it is critical, which is the opposite of the summary's case.** A reader that skipped a
//! summary replays the chain and lands where the summary said; a reader that skipped *this* would
//! apply an act that puts already-signed operations out of effect as though it were an ordinary
//! one, at an ordinary threshold. That is precisely *if you cannot read this you cannot claim to
//! have applied the act*.
//!
//! # What it costs to sign one
//!
//! **The most that object has** (`SPECS.md §4.9`), because discarding signed operations is not
//! routine. For an organisation that is the governance threshold — and where the owners cannot
//! reach it, the entity is the frozen case of `SPECS.md §12.3` and the way out is the emergency
//! continuity of `§8.3`. For a person there are no owners and no thresholds, so it is **a current
//! device**, which lands at once, or **the control key**, which waits its seventy-two epochs and
//! which any current device may cancel.
//!
//! That last asymmetry is what settles the ugly case. Two rival `rotate` acts from one predecessor
//! — the owner's and one made by somebody who copied the words — can both be followed by rival
//! resolutions. The thief's is signed by the words alone, so it waits and the owner's device
//! cancels it; the owner's is signed by a device and lands at once. The tie is broken by whoever
//! has an aparatus in their hand, which is exactly who it should be broken by.

use almena_format::cbor::Value;
use almena_format::identifier::Name;
use almena_format::operation::Operation;

/// Where a resolution rides.
///
/// Above [`almena_format::field::COMMON`], where a number means one thing whatever kind of act
/// carries it, and **odd**, so a build that cannot read it refuses the act rather than applying it
/// as something smaller.
pub const FIELD: u64 = almena_format::field::COMMON + 1;

/// The branch a resolution keeps, if the act declares one.
///
/// The name of the act it chains from — which is also its `prev`, and saying it twice is the point:
/// **an act that merely chained somewhere would be a resolution only to a reader that knew the
/// chain was forked**, and the whole design rests on validity being decidable from the act. Written
/// out, it says what it is to anybody at all, including the person about to sign it.
///
/// [`None`] where the act is not a resolution, which is nearly every act.
#[must_use]
pub fn keeps(operation: &Operation) -> Option<Name> {
    match operation.payload.get(&FIELD) {
        Some(Value::Text(named)) => Name::parse(named).ok(),
        _ => None,
    }
}

/// Whether this act declares itself a resolution and agrees with itself about which branch it keeps.
///
/// **The two have to match.** An act naming one branch and chaining from another would be one
/// whose own two halves disagree, and there is no reading of it that is safe to pick.
#[must_use]
pub fn declared(operation: &Operation) -> bool {
    match (keeps(operation), operation.previous.as_ref()) {
        (Some(kept), Some(previous)) => kept == *previous,
        // A creation cannot resolve anything: there is no fork before the first act.
        _ => false,
    }
}

/// The fields that mean one thing whatever kind of act carries them, and are critical.
///
/// **Every vocabulary has to include these**, because a kind's own list is what decides whether an
/// act can be applied — and a critical field left out of one is an act that kind can never carry.
/// `crates/almena-store/src/chain.rs` has the test that says so, so that adding a common field is
/// one edit and a failing test rather than four edits and a silence.
pub const COMMON: &[almena_format::field::Field] = &[almena_format::field::Field::new(FIELD)];

/// Put the field on an act, which is what makes it a resolution.
///
/// The caller passes the head it is chaining from, so that the act says the same thing twice and
/// [`declared`] holds.
pub fn declaring(operation: &mut Operation, keeping: &Name) {
    operation
        .payload
        .insert(FIELD, Value::Text(keeping.as_str().to_owned()));
}

#[cfg(test)]
mod tests {
    use super::{FIELD, declared, declaring, keeps};
    use almena_format::cbor::Value;
    use almena_format::identifier::{Did, Name, Network};
    use almena_format::operation::Operation;
    use almena_time::Epoch;
    use std::collections::BTreeMap;

    fn act(previous: Option<Name>) -> Operation {
        Operation {
            object: Did::new(Network::Development, Name::of(b"an object")),
            previous,
            kind: 2,
            version: 1,
            issued: Epoch::GENESIS,
            payload: BTreeMap::new(),
            signatures: Vec::new(),
        }
    }

    #[test]
    fn an_ordinary_act_declares_nothing() {
        let ordinary = act(Some(Name::of(b"what came before")));
        assert_eq!(keeps(&ordinary), None);
        assert!(!declared(&ordinary));
    }

    #[test]
    fn a_resolution_says_the_same_thing_twice_and_has_to() {
        // An act naming one branch and chaining from another is one whose own two halves disagree,
        // and there is no reading of it that is safe to pick.
        let head = Name::of(b"the branch that survives");
        let mut resolving = act(Some(head.clone()));
        declaring(&mut resolving, &head);
        assert_eq!(keeps(&resolving), Some(head));
        assert!(declared(&resolving));

        let mut crossed = act(Some(Name::of(b"one branch")));
        declaring(&mut crossed, &Name::of(b"another branch"));
        assert!(
            !declared(&crossed),
            "and disagreeing with itself is not one"
        );
    }

    #[test]
    fn a_creation_resolves_nothing_because_there_is_no_fork_before_the_first_act() {
        let mut first = act(None);
        declaring(&mut first, &Name::of(b"anything"));
        assert!(!declared(&first));
    }

    #[test]
    fn something_that_is_not_a_name_is_not_a_declaration() {
        let mut nonsense = act(Some(Name::of(b"before")));
        nonsense
            .payload
            .insert(FIELD, Value::Text("not a name at all".to_owned()));
        assert_eq!(keeps(&nonsense), None);
        assert!(!declared(&nonsense));

        let mut wrong = act(Some(Name::of(b"before")));
        wrong.payload.insert(FIELD, Value::Uint(9));
        assert_eq!(keeps(&wrong), None);
    }

    #[test]
    const fn it_rides_where_a_number_means_one_thing_whatever_carries_it() {
        // Above the line, so it does not collide with a kind's own numbering; and odd, so a build
        // that cannot read it refuses the act rather than applying it as something smaller.
        const { assert!(FIELD > almena_format::field::COMMON) };
        const { assert!(FIELD % 2 == 1) };
    }
}
