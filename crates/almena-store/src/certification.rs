//! The seal: one entity's signed statement that it has checked another.
//!
//! # It is an object of its own, and that is not a detail
//!
//! A certification does **not** live on the chain of the entity it is about (`SPECS.md §2.2`,
//! `§4.9`). It has a chain of its own, and points at its subject — for the same reason a vote does:
//! **nobody writes in somebody else's chain.** Were it otherwise, being certified would mean
//! letting another party append to your own history, and withdrawing a seal would mean editing it.
//!
//! # Anybody may certify anybody, and that is the design
//!
//! There is no privilege to delegate, so there is no permission to ask (`SPECS.md §7.3`). What a
//! certification is worth is decided by **who issued it** and by the reader, who chooses their own
//! root of trust. This module therefore refuses nothing on the grounds of who is doing the
//! certifying; what it holds to is that the statement is complete and says what it means.
//!
//! # Three grades and no more, from one closed vocabulary
//!
//! Above three they stop being distinguishable to whoever reads them, which is the same reason they
//! are not shown as a continuous gauge (`SPECS.md §7.2`). And the vocabulary is **the protocol's**
//! rather than each issuer's: if every entity named its own, a stranger's *high level* would be
//! read as Almena's.
//!
//! # A reason, always, and in both languages
//!
//! `SPECS.md §7.8` and `§7.10` both say it, and this is where it stops being advice: an act with no
//! reason, or with a reason in one language, is **refused**. A gate without a published reason is
//! arbitrariness — and a reason half the readers cannot read is not a published reason.

use std::collections::BTreeMap;

use almena_format::cbor::Value;
use almena_format::identifier::Did;
use almena_format::operation::Operation;
use almena_time::{Epoch, Epochs};

use crate::chain::Refused;
use crate::kind::Kind;

/// How long notice a withdrawal for non-compliance gives before it takes effect.
///
/// Thirty days (`SPECS.md §7.8`). **Not an urgency, and treating it as one turns a formality into
/// the end of somebody's business.** Risk is the other case and gets no notice at all, because an
/// issuer that has been compromised is doing harm now.
pub const NOTICE: Epochs = almena_time::deadline::GRADE_LOWERING_NOTICE;

/// Where each part of a certification act sits.
///
/// **All odd.** A reader that passed over the grade would hold a seal without knowing what was
/// checked; over the subject, a statement about nobody; over the reason, a decision with no
/// published ground — which `SPECS.md §7.10` makes the difference between a gate and arbitrariness.
pub mod field {
    /// Who the certification is about.
    pub const SUBJECT: u64 = 1;
    /// Which grade, from the closed vocabulary.
    pub const GRADE: u64 = 3;
    /// Why, in each language it is written in.
    pub const REASON: u64 = 5;
    /// Which cause a withdrawal is for.
    pub const CAUSE: u64 = 7;
    /// Who is issuing it.
    ///
    /// **Named in the act and checked against the record**, exactly as an element's parent is: what
    /// makes it true is that this entity's owners signed, at the threshold sealing costs
    /// (`SPECS.md §7.10`, `§8.2`). Without the field there would be nothing to check against, and
    /// deriving it from whoever signed would mean guessing which of an owner's organisations they
    /// were speaking for.
    pub const BY: u64 = 9;
}

/// What a certification says was checked.
///
/// **Three, and the vocabulary is closed** (`SPECS.md §7.2`): a number this build does not know is
/// refused rather than read as the nearest one it does, because *a grade slightly above the one I
/// know* is exactly the reading that would be dangerous.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Grade {
    /// A verified domain, and legal existence checked.
    Basic,
    /// That, and that whoever asked can represent the entity.
    Verified,
    /// That, and minimums of governance, and a node contributed.
    Reinforced,
}

impl Grade {
    /// The grade a number names, if it is one at all.
    #[must_use]
    pub const fn of(number: u64) -> Option<Self> {
        match number {
            1 => Some(Self::Basic),
            2 => Some(Self::Verified),
            3 => Some(Self::Reinforced),
            _ => None,
        }
    }

    /// The number it travels as.
    #[must_use]
    pub const fn number(self) -> u64 {
        match self {
            Self::Basic => 1,
            Self::Verified => 2,
            Self::Reinforced => 3,
        }
    }
}

/// Why a seal was withdrawn.
///
/// **The two are not the same decision and must not move at the same speed** (`SPECS.md §7.8`). One
/// is an emergency and the other is a formality, and treating the second as the first is how a
/// procedure becomes the end of somebody's business.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cause {
    /// Something is doing harm now. Immediate, with no notice.
    Risk,
    /// Something is not being kept to. Notice first, and then it takes effect.
    NonCompliance,
}

impl Cause {
    /// The cause a number names.
    #[must_use]
    pub const fn of(number: u64) -> Option<Self> {
        match number {
            1 => Some(Self::Risk),
            2 => Some(Self::NonCompliance),
            _ => None,
        }
    }

    /// The number it travels as.
    #[must_use]
    pub const fn number(self) -> u64 {
        match self {
            Self::Risk => 1,
            Self::NonCompliance => 2,
        }
    }

    /// When a withdrawal for this cause takes effect, counted from the act.
    #[must_use]
    pub fn takes_effect(self, at: Epoch) -> Epoch {
        match self {
            Self::Risk => at,
            Self::NonCompliance => at.plus(NOTICE).unwrap_or(Epoch::new(u64::MAX)),
        }
    }
}

/// The languages a reason has to be written in for it to count as published.
///
/// **The two the platform ships in** (`SPECS.md §13.9`). It is a floor rather than a list of the
/// only languages allowed: a reason may carry more, and it may not carry fewer.
pub const AT_LEAST: [&str; 2] = ["en", "es"];

/// Why a decision was taken, in every language it was written in.
///
/// **A list of pairs and not a map**, because this format's maps are keyed by unsigned integers and
/// a language tag is not one. The pairs are held to **strictly ascending tags** for the same reason
/// map keys are: canonical order is part of what was signed, and two orders of the same reason would
/// be two byte strings carrying one statement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reason(BTreeMap<String, String>);

impl Reason {
    /// Read one, refusing anything that is not a published reason.
    ///
    /// # Errors
    ///
    /// [`Refused::Malformed`] for a reason that is missing, empty, out of order, or written in
    /// fewer languages than `SPECS.md §13.9` asks for. **Refused and not repaired**: a gate with no
    /// published reason is arbitrariness, and one in a language half the readers cannot read is not
    /// a published reason.
    pub fn read(operation: &Operation) -> Result<Self, Refused> {
        Self::read_at(operation, field::REASON)
    }

    /// The same, from whichever field carries it.
    ///
    /// **One reader for both**, because a reply is held to the same floor the decision was: an
    /// answer half the readers cannot read is not published beside anything.
    ///
    /// # Errors
    ///
    /// As [`Self::read`].
    pub fn read_at(operation: &Operation, at: u64) -> Result<Self, Refused> {
        let Some(Value::Array(pairs)) = operation.payload.get(&at) else {
            return Err(Refused::Malformed);
        };
        let mut said = BTreeMap::new();
        let mut last: Option<&str> = None;
        for pair in pairs {
            let Value::Array(pair) = pair else {
                return Err(Refused::Malformed);
            };
            let [Value::Text(tag), Value::Text(text)] = pair.as_slice() else {
                return Err(Refused::Malformed);
            };
            // **Empty is not written in that language.** A tag with nothing behind it would let a
            // reason satisfy the floor while saying nothing to half the people it is for.
            if tag.is_empty() || text.trim().is_empty() {
                return Err(Refused::Malformed);
            }
            if last.is_some_and(|before| before >= tag.as_str()) {
                return Err(Refused::Malformed);
            }
            last = Some(tag);
            said.insert(tag.clone(), text.clone());
        }
        if !AT_LEAST.iter().all(|tag| said.contains_key(*tag)) {
            return Err(Refused::Malformed);
        }
        Ok(Self(said))
    }

    /// Write one out, in the one order it may be written in.
    #[must_use]
    pub fn carried(said: &BTreeMap<String, String>) -> Value {
        Value::Array(
            said.iter()
                .map(|(tag, text)| {
                    Value::Array(vec![Value::Text(tag.clone()), Value::Text(text.clone())])
                })
                .collect(),
        )
    }

    /// What it says in that language, if it says anything.
    #[must_use]
    pub fn in_language(&self, tag: &str) -> Option<&str> {
        self.0.get(tag).map(String::as_str)
    }

    /// Every language it was written in.
    #[must_use]
    pub fn languages(&self) -> Vec<&str> {
        self.0.keys().map(String::as_str).collect()
    }
}

/// One entity's signed statement about another.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Certification {
    /// Who issued it, which is what decides what it is worth.
    pub by: Did,
    /// Who it is about.
    pub subject: Did,
    /// What was checked.
    pub grade: Grade,
    /// The epoch it was issued in.
    pub since: Epoch,
    /// Why it was issued, published for anybody to read.
    pub reason: Reason,
    /// When it was withdrawn and why, if it was.
    pub withdrawn: Option<Withdrawn>,
}

/// A seal taken back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Withdrawn {
    /// Which cause, which decides whether it is immediate.
    pub cause: Cause,
    /// The first epoch at which it no longer stands.
    pub from: Epoch,
    /// Why, published beside the decision.
    pub reason: Reason,
}

impl Certification {
    /// Whether it still stands at that moment.
    ///
    /// **Never retroactive** (`SPECS.md §4.3`, `§7.3`): what was signed while the seal stood goes on
    /// being valid, evaluated against the moment of the act. So this asks about a moment rather than
    /// about now, and a withdrawal changes what happens from its own date forward and nothing before.
    #[must_use]
    pub fn stands(&self, at: Epoch) -> bool {
        if at.number() < self.since.number() {
            return false;
        }
        self.withdrawn
            .as_ref()
            .is_none_or(|gone| at.number() < gone.from.number())
    }
}

/// The fields a certification act may carry that this build has a meaning for.
#[must_use]
pub fn vocabulary() -> almena_format::field::Vocabulary<'static> {
    use almena_format::field::Field;
    const FIELDS: &[Field] = &[
        Field::new(field::SUBJECT),
        Field::new(field::GRADE),
        Field::new(field::REASON),
        Field::new(field::CAUSE),
        Field::new(field::BY),
        Field::new(crate::resolution::FIELD),
    ];
    const CLOSED: &[(Field, &[Value])] = &[
        // **The grade's vocabulary is closed** (`SPECS.md §7.2`): a number this build does not know
        // is refused rather than read as the nearest one it does, because *a grade slightly above
        // the one I know* is exactly the reading that would be dangerous.
        (
            Field::new(field::GRADE),
            &[Value::Uint(1), Value::Uint(2), Value::Uint(3)],
        ),
        (Field::new(field::CAUSE), &[Value::Uint(1), Value::Uint(2)]),
    ];
    almena_format::field::Vocabulary::with_closed(FIELDS, CLOSED)
}

/// Who a certification is about, read from the act itself.
///
/// **What makes it findable by the party affected** rather than by whoever bothered to write it
/// down, which is the whole of what a subject is for.
#[must_use]
pub fn about(operation: &Operation) -> Option<Did> {
    match Kind::new(operation.kind) {
        Some(Kind::CERTIFICATION_ISSUE | Kind::CERTIFICATION_REVOKE) => {
            match operation.payload.get(&field::SUBJECT) {
                Some(Value::Text(subject)) => Did::parse(subject).ok(),
                _ => None,
            }
        }
        _ => None,
    }
}

/// Who is issuing a certification, read from the act.
#[must_use]
pub fn issuer(operation: &Operation) -> Option<Did> {
    match operation.payload.get(&field::BY) {
        Some(Value::Text(by)) => Did::parse(by).ok(),
        _ => None,
    }
}

/// A certification, as the act that issued it made it.
///
/// **`by` is whoever signed it**, and this module refuses nothing on those grounds: anybody may
/// certify anybody (`SPECS.md §7.3`). What a certification is worth is decided by who issued it and
/// by the reader, who chooses their own root of trust.
///
/// # Errors
///
/// [`Refused::Malformed`] for an act with no subject, no grade this build knows, or no reason
/// published in the languages `SPECS.md §13.9` asks for.
pub fn born(operation: &Operation) -> Result<Certification, Refused> {
    let by = issuer(operation).ok_or(Refused::Malformed)?;
    let subject = about(operation).ok_or(Refused::Malformed)?;
    // **A certification about its own issuer is not one.** It would be a party vouching for itself,
    // which says nothing and would be read by somebody as though it said something.
    if subject == by {
        return Err(Refused::NotAuthorised);
    }
    let grade = match operation.payload.get(&field::GRADE) {
        Some(Value::Uint(number)) => Grade::of(*number).ok_or(Refused::Malformed)?,
        _ => return Err(Refused::Malformed),
    };
    Ok(Certification {
        by,
        subject,
        grade,
        since: operation.issued,
        reason: Reason::read(operation)?,
        withdrawn: None,
    })
}

/// What an act does to a certification.
///
/// # Errors
///
/// [`Refused`].
pub fn does(
    operation: &Operation,
    certification: &Certification,
    kind: Kind,
) -> Result<Certification, Refused> {
    let mut next = certification.clone();
    match kind {
        Kind::CERTIFICATION_REVOKE => {
            // **Never retroactive** (`SPECS.md §4.3`, `§7.3`). What was signed while the seal stood
            // goes on being valid; what changes is what happens from here forward.
            let cause = match operation.payload.get(&field::CAUSE) {
                Some(Value::Uint(number)) => Cause::of(*number).ok_or(Refused::Malformed)?,
                _ => return Err(Refused::Malformed),
            };
            // **Said once and not moved.** A second withdrawal that brought a date forward would be
            // a notice period somebody could shorten after announcing it.
            if next.withdrawn.is_some() {
                return Err(Refused::NotAuthorised);
            }
            next.withdrawn = Some(Withdrawn {
                cause,
                from: cause.takes_effect(operation.issued),
                reason: Reason::read(operation)?,
            });
        }
        _ => return Err(Refused::Malformed),
    }
    Ok(next)
}

#[cfg(test)]
mod tests {
    use super::{AT_LEAST, Cause, Certification, Grade, NOTICE, Reason, about, born, does};
    use crate::chain::Refused;
    use crate::kind::Kind;
    use almena_format::cbor::Value;
    use almena_format::identifier::{Did, Name, Network};
    use almena_format::operation::{Operation, create};
    use almena_time::Epoch;
    use std::collections::BTreeMap;

    fn now() -> Epoch {
        Epoch::new(100)
    }

    fn who(seed: u8) -> Did {
        Did::new(Network::Development, Name::of(&[seed; 8]))
    }

    /// A reason, in the languages it has to be written in.
    fn reason(languages: &[&str]) -> Value {
        Reason::carried(
            &languages
                .iter()
                .map(|tag| ((*tag).to_owned(), format!("the reason, in {tag}")))
                .collect(),
        )
    }

    fn issuing(subject: &Did, grade: u64, reason: Value) -> Operation {
        by_whom(&who(1), subject, grade, reason)
    }

    fn by_whom(by: &Did, subject: &Did, grade: u64, reason: Value) -> Operation {
        create(
            Network::Development,
            Kind::CERTIFICATION_ISSUE.number(),
            1,
            now(),
            BTreeMap::from([
                (super::field::SUBJECT, Value::Text(subject.to_string())),
                (super::field::GRADE, Value::Uint(grade)),
                (super::field::REASON, reason),
                (super::field::BY, Value::Text(by.to_string())),
            ]),
        )
    }

    fn revoking(subject: &Did, cause: Cause) -> Operation {
        Operation {
            object: who(9),
            previous: Some(Name::of(b"the issuing")),
            kind: Kind::CERTIFICATION_REVOKE.number(),
            version: 1,
            issued: now(),
            payload: BTreeMap::from([
                (super::field::SUBJECT, Value::Text(subject.to_string())),
                (super::field::CAUSE, Value::Uint(cause.number())),
                (super::field::REASON, reason(&AT_LEAST)),
            ]),
            signatures: Vec::new(),
        }
    }

    fn a_certification() -> Certification {
        born(&issuing(&who(2), 1, reason(&AT_LEAST))).expect("complete")
    }

    #[test]
    fn a_decision_with_no_published_reason_is_not_taken() {
        // **A gate with no published reason is arbitrariness** (`SPECS.md §7.10`), and one written
        // in a language half the readers cannot read is not a published reason (`§13.9`).
        for missing in [
            reason(&["en"]),
            reason(&["es"]),
            reason(&[]),
            Value::Text("just a sentence".to_owned()),
        ] {
            assert_eq!(born(&issuing(&who(2), 1, missing)), Err(Refused::Malformed));
        }
        assert!(born(&issuing(&who(2), 1, reason(&AT_LEAST))).is_ok());
    }

    #[test]
    fn a_reason_with_a_language_and_nothing_behind_it_is_not_written_in_it() {
        // Otherwise a reason satisfies the floor while saying nothing to half the people it is for.
        let empty = Value::Array(vec![
            Value::Array(vec![
                Value::Text("en".to_owned()),
                Value::Text("   ".to_owned()),
            ]),
            Value::Array(vec![
                Value::Text("es".to_owned()),
                Value::Text("un motivo".to_owned()),
            ]),
        ]);
        assert_eq!(born(&issuing(&who(2), 1, empty)), Err(Refused::Malformed));
    }

    #[test]
    fn a_reason_written_in_two_orders_would_be_two_statements() {
        // Canonical order is part of what was signed, exactly as it is for map keys.
        let backwards = Value::Array(vec![
            Value::Array(vec![
                Value::Text("es".to_owned()),
                Value::Text("un motivo".to_owned()),
            ]),
            Value::Array(vec![
                Value::Text("en".to_owned()),
                Value::Text("a reason".to_owned()),
            ]),
        ]);
        assert_eq!(
            born(&issuing(&who(2), 1, backwards)),
            Err(Refused::Malformed)
        );
    }

    #[test]
    fn a_grade_this_build_does_not_know_is_refused_and_not_read_as_the_nearest_one() {
        // *A grade slightly above the one I know* is exactly the reading that would be dangerous.
        assert_eq!(Grade::of(4), None);
        assert_eq!(
            born(&issuing(&who(2), 4, reason(&AT_LEAST))),
            Err(Refused::Malformed)
        );
        for (number, grade) in [
            (1, Grade::Basic),
            (2, Grade::Verified),
            (3, Grade::Reinforced),
        ] {
            assert_eq!(Grade::of(number), Some(grade));
            assert_eq!(grade.number(), number);
        }
    }

    #[test]
    fn a_party_does_not_vouch_for_itself() {
        // It would say nothing, and somebody would read it as though it said something.
        assert_eq!(
            born(&by_whom(&who(1), &who(1), 1, reason(&AT_LEAST))),
            Err(Refused::NotAuthorised)
        );
    }

    #[test]
    fn withdrawing_for_risk_is_immediate_and_for_non_compliance_gives_notice() {
        // **Two decisions that must not move at the same speed** (`SPECS.md §7.8`): one is an
        // emergency, the other a formality, and treating the second as the first is how a procedure
        // becomes the end of somebody's business.
        let held = a_certification();
        let at_once = does(
            &revoking(&who(2), Cause::Risk),
            &held,
            Kind::CERTIFICATION_REVOKE,
        )
        .expect("signed");
        assert!(!at_once.stands(now()));

        let noticed = does(
            &revoking(&who(2), Cause::NonCompliance),
            &held,
            Kind::CERTIFICATION_REVOKE,
        )
        .expect("signed");
        assert!(noticed.stands(now()), "not yet");
        assert!(!noticed.stands(Epoch::new(now().number() + NOTICE.count())));
    }

    #[test]
    fn what_was_signed_while_the_seal_stood_goes_on_standing() {
        // **Never retroactive** (`SPECS.md §4.3`, `§7.3`): validity is evaluated against the moment
        // of the act, so a withdrawal changes what happens forward and nothing behind it.
        let gone = does(
            &revoking(&who(2), Cause::Risk),
            &a_certification(),
            Kind::CERTIFICATION_REVOKE,
        )
        .expect("signed");
        assert!(
            !gone.stands(Epoch::new(now().number() - 1)),
            "and it did not stand before it was issued either"
        );
    }

    #[test]
    fn a_withdrawal_is_said_once_and_its_date_is_not_moved_afterwards() {
        // A second one bringing the date forward would be a notice period somebody could shorten
        // after announcing it.
        let noticed = does(
            &revoking(&who(2), Cause::NonCompliance),
            &a_certification(),
            Kind::CERTIFICATION_REVOKE,
        )
        .expect("signed");
        assert_eq!(
            does(
                &revoking(&who(2), Cause::Risk),
                &noticed,
                Kind::CERTIFICATION_REVOKE
            ),
            Err(Refused::NotAuthorised)
        );
    }

    #[test]
    fn a_certification_says_who_it_is_about_so_the_party_affected_can_find_it() {
        let subject = who(2);
        assert_eq!(
            about(&issuing(&subject, 1, reason(&AT_LEAST))),
            Some(subject)
        );
    }

    #[test]
    fn what_the_reason_says_is_readable_in_each_language_it_was_written_in() {
        let held = a_certification();
        assert_eq!(held.reason.languages(), AT_LEAST);
        assert_eq!(held.reason.in_language("en"), Some("the reason, in en"));
        assert_eq!(held.reason.in_language("fr"), None);
    }
}
