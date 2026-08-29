//! What a request is for, from a list nobody may extend on their own.
//!
//! # Closed, and that is the whole mechanism
//!
//! A presentation template carries one or more tags — *age verification*, *opening an account*,
//! *access to content* — and the catalogue groups by them, so that what one party asks for can be
//! read beside what everybody else asking for the same thing asks for (`SPECS.md §9.4`).
//!
//! **If anybody could invent one, declaring your own would be enough to be compared with nobody.**
//! That is the whole of why the list is closed: with a closed list you cannot fall outside the
//! taxonomy — you either carry relevant tags or you carry none, and **carrying none shows**.
//!
//! Tags rather than categories, because a template can genuinely serve more than one end.
//!
//! # Almena Government's, at the governance threshold, and only on two parties' asking
//!
//! **A tag is added when two independent parties need it, never on one party's request**
//! (`SPECS.md §9.4`). That is what avoids both proliferation and the tag made to measure so that
//! its author is compared with nobody. Whether two parties asked is not something the record can
//! check — it is why the list has a keeper — so what the record holds is the decision and who took
//! it, and the reason is published like every other decision.
//!
//! # In every language the platform ships in
//!
//! Like the core of attributes, and for the same reason: Almena maintains these, so the obligation
//! to translate falls on Almena and on nobody else (`SPECS.md §9.4`). It is what keeps the
//! *untranslated* mark rare enough to still be read.

use almena_format::cbor::Value;
use almena_format::identifier::Did;
use almena_format::operation::Operation;
use almena_time::Epoch;

use crate::attribute::Written;
use crate::chain::Refused;
use crate::kind::Kind;

/// The languages a tag has to be readable in.
///
/// **Every language the platform ships in** — the one translation obligation that exists, and it
/// falls on whoever maintains the list rather than on the ecosystem (`SPECS.md §9.4`).
pub const IN_ALL: [&str; 2] = ["en", "es"];

/// Where each part of a tag act sits.
pub mod field {
    /// What it is called, which is what a template names.
    pub const NAME: u64 = 1;
    /// The label a person reads, in each language it is written in.
    pub const LABELS: u64 = 3;
    /// Who is adding it.
    pub const BY: u64 = 5;
}

/// One purpose a template may be classified under.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tag {
    /// What it is called.
    pub name: String,
    /// The label a person reads, in each language it has been written in.
    pub labels: Written,
    /// Who added it.
    pub by: Did,
    /// The epoch it stopped being one a new template may carry, if it has.
    pub deprecated: Option<Epoch>,
}

impl Tag {
    /// Whether a new template may still carry it at that moment.
    #[must_use]
    pub fn usable(&self, at: Epoch) -> bool {
        self.deprecated
            .is_none_or(|since| at.number() < since.number())
    }
}

/// Who is adding a tag, read from the act.
#[must_use]
pub fn adding(operation: &Operation) -> Option<Did> {
    match operation.payload.get(&field::BY) {
        Some(Value::Text(by)) => Did::parse(by).ok(),
        _ => None,
    }
}

/// The fields a tag act may carry that this build has a meaning for.
#[must_use]
pub fn vocabulary() -> almena_format::field::Vocabulary<'static> {
    use almena_format::field::Field;
    const FIELDS: &[Field] = &[
        Field::new(field::NAME),
        Field::new(field::LABELS),
        Field::new(field::BY),
        Field::new(crate::resolution::FIELD),
    ];
    almena_format::field::Vocabulary::of(FIELDS)
}

/// A tag, as the act that added it made it.
///
/// # Errors
///
/// [`Refused::Malformed`] for an act with no name, no adder, or labels in fewer languages than the
/// platform ships in — because the one translation obligation there is falls here.
pub fn born(operation: &Operation) -> Result<Tag, Refused> {
    let labels = crate::attribute::written_at(operation, field::LABELS)?;
    if !IN_ALL.iter().all(|tag| labels.contains_key(*tag)) {
        return Err(Refused::Malformed);
    }
    Ok(Tag {
        name: crate::attribute::text_at(operation, field::NAME)?,
        labels,
        by: adding(operation).ok_or(Refused::Malformed)?,
        deprecated: None,
    })
}

/// What an act does to a tag.
///
/// # Errors
///
/// [`Refused`].
pub fn does(operation: &Operation, tag: &Tag, kind: Kind) -> Result<Tag, Refused> {
    let mut next = tag.clone();
    match kind {
        Kind::TAG_TRANSLATE => {
            let more = crate::attribute::written_at(operation, field::LABELS)?;
            if more.is_empty() {
                return Err(Refused::Malformed);
            }
            next.labels.extend(more);
        }
        Kind::TAG_DEPRECATE => {
            if next.deprecated.is_some() {
                return Err(Refused::NotAuthorised);
            }
            next.deprecated = Some(operation.issued);
        }
        _ => return Err(Refused::Malformed),
    }
    Ok(next)
}

#[cfg(test)]
mod tests {
    use super::{IN_ALL, born, does, field};
    use crate::attribute::{Written, carried};
    use crate::chain::Refused;
    use crate::kind::Kind;
    use almena_format::cbor::Value;
    use almena_format::identifier::{Did, Name, Network};
    use almena_format::operation::{Operation, create};
    use almena_time::Epoch;
    use std::collections::BTreeMap;

    fn at() -> Epoch {
        Epoch::new(100)
    }

    fn almena() -> Did {
        Did::new(Network::Development, Name::of(b"almena government"))
    }

    fn labels(languages: &[&str]) -> Value {
        carried(
            &languages
                .iter()
                .map(|tag| ((*tag).to_owned(), format!("age verification, in {tag}")))
                .collect::<Written>(),
        )
    }

    fn added(extra: &[(u64, Value)]) -> Operation {
        let mut payload = BTreeMap::from([
            (field::NAME, Value::Text("age-verification".to_owned())),
            (field::LABELS, labels(&IN_ALL)),
            (field::BY, Value::Text(almena().to_string())),
        ]);
        for (which, value) in extra {
            payload.insert(*which, value.clone());
        }
        create(
            Network::Development,
            Kind::TAG_ADD.number(),
            1,
            at(),
            payload,
        )
    }

    #[test]
    fn a_tag_is_readable_in_every_language_the_platform_ships_in() {
        // **The one translation obligation there is**, and it falls on whoever maintains the list
        // rather than on the ecosystem (`SPECS.md §9.4`) — which is what keeps the *untranslated*
        // mark rare enough to still be read.
        let tag = born(&added(&[])).expect("complete");
        assert_eq!(tag.name, "age-verification");
        assert!(tag.usable(at()));

        for half in [labels(&["en"]), labels(&["es"]), labels(&[])] {
            assert_eq!(
                born(&added(&[(field::LABELS, half)])),
                Err(Refused::Malformed)
            );
        }
    }

    #[test]
    fn a_tag_with_no_name_is_not_one_a_template_can_carry() {
        for missing in [field::NAME, field::LABELS, field::BY] {
            let mut act = added(&[]);
            act.payload.remove(&missing);
            assert_eq!(born(&act), Err(Refused::Malformed), "field {missing}");
        }
    }

    #[test]
    fn a_language_can_be_added_later_without_the_tag_becoming_another_tag() {
        let tag = born(&added(&[])).expect("complete");
        let mut translating = added(&[(field::LABELS, labels(&["fr"]))]);
        translating.kind = Kind::TAG_TRANSLATE.number();
        translating
            .payload
            .insert(field::NAME, Value::Text("something-else".to_owned()));

        let after = does(&translating, &tag, Kind::TAG_TRANSLATE).expect("signed");
        assert_eq!(after.name, "age-verification", "what it is did not move");
        assert_eq!(after.labels.len(), 3);
    }

    #[test]
    fn deprecating_reaches_only_templates_not_yet_published() {
        let tag = born(&added(&[])).expect("complete");
        let later = Epoch::new(at().number() + 500);
        let mut gone = added(&[]);
        gone.kind = Kind::TAG_DEPRECATE.number();
        gone.issued = later;

        let after = does(&gone, &tag, Kind::TAG_DEPRECATE).expect("signed");
        assert!(after.usable(at()));
        assert!(!after.usable(later));
    }
}
