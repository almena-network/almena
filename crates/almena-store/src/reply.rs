//! The right of reply: what the party a decision was taken about has to say back.
//!
//! # Why this exists at all, and why it is not an appeal
//!
//! There is **no authority above Almena** (`SPECS.md §7.8`), so appealing *to Almena* is asking it
//! to re-read itself, which is not an appeal. Pretending otherwise would be worse than saying so.
//!
//! What fits the rest of the design is that **the decision and the answer are published together,
//! and for ever**. Whoever chooses their own root of trust — every consumer, by `SPECS.md §1.5` —
//! reads both and judges. If Almena decides badly and does it repeatedly, what loses value is its
//! seal, and that is the only real corrective in a place where nobody is above anybody.
//!
//! # An object of its own, and it has to be
//!
//! A reply does not live on the chain of the decision it answers. It points at it, the way a vote
//! points at a proposal and a certification points at its subject — because **nobody writes in
//! somebody else's chain**. Appended to Almena's object, a reply would mean the party affected can
//! add to what Almena said; needing Almena's agreement, it would be no reply at all.
//!
//! # Nobody moderates it, and nobody is required to like it
//!
//! Whoever the decision was about may publish one, and this refuses nothing about what it says. A
//! right of reply that its subject could withhold would be a right its subject grants, which is the
//! one thing it must not be.

use almena_format::cbor::Value;
use almena_format::identifier::{Did, Name};
use almena_format::operation::Operation;
use almena_time::Epoch;

use crate::certification::Reason;
use crate::chain::Refused;

/// Where each part of a reply sits.
///
/// **Both odd.** A reader that passed over what it answers would hold a statement about nothing;
/// over what it says, a reply that says nothing — which is worse than no reply, because it would be
/// counted as one.
pub mod field {
    /// Which decision it answers.
    pub const TO: u64 = 1;
    /// What it says, in each language it is written in.
    pub const SAID: u64 = 3;
}

/// One answer to one decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reply {
    /// The decision it answers.
    pub to: Name,
    /// Who is answering, which is the party the decision was about.
    pub by: Did,
    /// What they say, published beside the decision.
    pub said: Reason,
    /// The epoch it was published in.
    pub at: Epoch,
}

/// Which decision an act answers, read from the act itself.
#[must_use]
pub fn answers(operation: &Operation) -> Option<Name> {
    match operation.payload.get(&field::TO) {
        Some(Value::Text(to)) => Name::parse(to).ok(),
        _ => None,
    }
}

/// The fields a reply may carry that this build has a meaning for.
#[must_use]
pub fn vocabulary() -> almena_format::field::Vocabulary<'static> {
    use almena_format::field::Field;
    const FIELDS: &[Field] = &[
        Field::new(field::TO),
        Field::new(field::SAID),
        Field::new(crate::resolution::FIELD),
    ];
    almena_format::field::Vocabulary::of(FIELDS)
}

/// A reply, as the act that published it made it.
///
/// `by` is the party the decision was about, resolved from the decision by whoever holds the record
/// — never taken from the act, because an act that named its own author would be an act letting
/// anybody answer in somebody else's name.
///
/// # Errors
///
/// [`Refused::Malformed`] for an act with no decision to answer, or nothing to say in the languages
/// `SPECS.md §13.9` asks for — **the same floor the decision itself had to clear**, because an
/// answer half the readers cannot read is not published beside anything.
pub fn born(operation: &Operation, by: Did) -> Result<Reply, Refused> {
    Ok(Reply {
        to: answers(operation).ok_or(Refused::Malformed)?,
        by,
        said: Reason::read_at(operation, field::SAID)?,
        at: operation.issued,
    })
}

#[cfg(test)]
mod tests {
    use super::{answers, born, field};
    use crate::certification::Reason;
    use crate::chain::Refused;
    use crate::kind::Kind;
    use almena_format::cbor::Value;
    use almena_format::identifier::{Did, Name, Network};
    use almena_format::operation::{Operation, create};
    use almena_time::Epoch;
    use std::collections::BTreeMap;

    fn who() -> Did {
        Did::new(Network::Development, Name::of(b"the party affected"))
    }

    fn decision() -> Name {
        Name::of(b"the decision")
    }

    fn said(languages: &[&str]) -> Value {
        Reason::carried(
            &languages
                .iter()
                .map(|tag| ((*tag).to_owned(), format!("what we say, in {tag}")))
                .collect(),
        )
    }

    fn replying(to: &Name, said: Value) -> Operation {
        create(
            Network::Development,
            Kind::REPLY_PUBLISH.number(),
            1,
            Epoch::new(50),
            BTreeMap::from([
                (field::TO, Value::Text(to.as_str().to_owned())),
                (field::SAID, said),
            ]),
        )
    }

    #[test]
    fn a_reply_says_which_decision_it_answers_so_the_two_are_read_together() {
        let published = born(&replying(&decision(), said(&["en", "es"])), who()).expect("complete");
        assert_eq!(published.to, decision());
        assert_eq!(published.by, who());
        assert_eq!(published.said.in_language("en"), Some("what we say, in en"));
    }

    #[test]
    fn an_answer_half_the_readers_cannot_read_is_not_published_beside_anything() {
        // **The same floor the decision itself had to clear** (`SPECS.md §7.8`, `§13.9`). A reply
        // in one language published beside a reason in two would leave half the readers with only
        // one side of it.
        for half in [said(&["en"]), said(&["es"]), said(&[])] {
            assert_eq!(
                born(&replying(&decision(), half), who()),
                Err(Refused::Malformed)
            );
        }
    }

    #[test]
    fn a_reply_that_answers_nothing_is_not_one() {
        let nowhere = create(
            Network::Development,
            Kind::REPLY_PUBLISH.number(),
            1,
            Epoch::new(50),
            BTreeMap::from([(field::SAID, said(&["en", "es"]))]),
        );
        assert_eq!(answers(&nowhere), None);
        assert_eq!(born(&nowhere, who()), Err(Refused::Malformed));
    }
}
