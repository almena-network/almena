//! Where a definition may be copied from.
//!
//! # An attribute is not invented here
//!
//! `SPECS.md §9.4` is firm about it: attributes come from public schemas, and a template references
//! them rather than defining them. A source is the record's way of saying **which schemas** — its
//! name, where it canonically lives, which version was admitted, and whether it still is.
//!
//! # Fix and copy, never depend live
//!
//! This is the rule the whole idea rests on. An external schema changes without warning, is not
//! signed, is not in the record, and cannot be resolved without going out to a host nobody chose.
//! So the register **fixes the version and copies the definition in**, and the external reference
//! stays as *provenance*. Otherwise somebody else's change would reinterpret credentials already
//! issued, which `SPECS.md §4.3` forbids, and would add a dependency `§4.6` never contemplated.
//!
//! Two things fall out of that, and both are worth having:
//!
//! - **Deprecating a source breaks nothing.** Attributes already published copied their
//!   definitions, so they are self-sufficient; what a deprecation changes is only what may be
//!   brought in from there next.
//! - **A source disappearing breaks nothing either**, for the same reason. What is lost is the
//!   possibility of new definitions from it.
//!
//! # Almena Government's, and at the governance threshold
//!
//! Admitting a source **changes the rules of the ecosystem** rather than attesting to a fact, so it
//! goes with the high threshold and not the sealing one (`SPECS.md §8.2`, `§9.4`). And it is work
//! with a cadence rather than a list written once: when OpenID Connect or the European PID publish
//! a new version, somebody has to decide whether it is admitted. A list of sources nobody revisits
//! leaves the ecosystem adrift from the standards it says it follows.

use almena_format::cbor::Value;
use almena_format::identifier::Did;
use almena_format::operation::Operation;
use almena_time::Epoch;

use crate::chain::Refused;
use crate::kind::Kind;

/// Where each part of a source act sits.
///
/// **All odd but one.** A reader that passed over which version was admitted would hold a source
/// that says nothing about what it admits; over where it lives, one nobody can go and check.
pub mod field {
    /// What it is called, as its own community calls it.
    pub const NAME: u64 = 1;
    /// A note about it, in each language it is written in.
    ///
    /// **Even**: a reader that passes over it holds the source and not the sentence about it, which
    /// is a poorer catalogue page and never a wrong claim.
    pub const ABOUT: u64 = 2;
    /// Where it canonically lives.
    pub const AT: u64 = 3;
    /// Which version of it was admitted.
    pub const VERSION: u64 = 5;
    /// Who is admitting it.
    pub const BY: u64 = 9;
}

/// One place definitions may be copied from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    /// What it is called.
    pub name: String,
    /// Where it canonically lives.
    pub at: String,
    /// Which version was admitted.
    pub version: String,
    /// Who admitted it.
    pub by: Did,
    /// The epoch it stopped being one to bring new definitions from, if it has.
    ///
    /// **Never retroactive** (`SPECS.md §4.3`, `§9.4`): attributes already published copied their
    /// definitions and are self-sufficient, so this reaches only what has not been published yet.
    pub deprecated: Option<Epoch>,
}

impl Source {
    /// Whether new definitions may still be brought in from it at that moment.
    #[must_use]
    pub fn admits(&self, at: Epoch) -> bool {
        self.deprecated
            .is_none_or(|since| at.number() < since.number())
    }
}

/// Who is admitting a source, read from the act.
#[must_use]
pub fn admitting(operation: &Operation) -> Option<Did> {
    match operation.payload.get(&field::BY) {
        Some(Value::Text(by)) => Did::parse(by).ok(),
        _ => None,
    }
}

/// The fields a source act may carry that this build has a meaning for.
#[must_use]
pub fn vocabulary() -> almena_format::field::Vocabulary<'static> {
    use almena_format::field::Field;
    const FIELDS: &[Field] = &[
        Field::new(field::NAME),
        Field::new(field::AT),
        Field::new(field::VERSION),
        Field::new(field::BY),
        Field::new(crate::resolution::FIELD),
    ];
    almena_format::field::Vocabulary::of(FIELDS)
}

/// A source, as the act that admitted it made it.
///
/// # Errors
///
/// [`Refused::Malformed`] for an act missing anything a source is: a name, somewhere it lives, the
/// version admitted, or who admitted it.
pub fn born(operation: &Operation) -> Result<Source, Refused> {
    Ok(Source {
        name: written(operation, field::NAME)?,
        at: written(operation, field::AT)?,
        version: written(operation, field::VERSION)?,
        by: admitting(operation).ok_or(Refused::Malformed)?,
        deprecated: None,
    })
}

/// What an act does to a source.
///
/// # Errors
///
/// [`Refused`].
pub fn does(operation: &Operation, source: &Source, kind: Kind) -> Result<Source, Refused> {
    let mut next = source.clone();
    match kind {
        Kind::SOURCE_DEPRECATE => {
            // **Said once.** A second deprecation moving the date would be a decision somebody
            // could revise after everybody had read it.
            if next.deprecated.is_some() {
                return Err(Refused::NotAuthorised);
            }
            next.deprecated = Some(operation.issued);
        }
        _ => return Err(Refused::Malformed),
    }
    Ok(next)
}

/// One text field, which has to be there and has to say something.
fn written(operation: &Operation, at: u64) -> Result<String, Refused> {
    match operation.payload.get(&at) {
        Some(Value::Text(text)) if !text.trim().is_empty() => Ok(text.clone()),
        _ => Err(Refused::Malformed),
    }
}

#[cfg(test)]
mod tests {
    use super::{admitting, born, does, field};
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

    fn admitted(extra: &[(u64, Value)]) -> Operation {
        let mut payload = BTreeMap::from([
            (field::NAME, Value::Text("OpenID Connect".to_owned())),
            (
                field::AT,
                Value::Text("https://openid.net/specs/openid-connect-core-1_0.html".to_owned()),
            ),
            (field::VERSION, Value::Text("1.0 errata 2".to_owned())),
            (field::BY, Value::Text(almena().to_string())),
        ]);
        for (which, value) in extra {
            payload.insert(*which, value.clone());
        }
        create(
            Network::Development,
            Kind::SOURCE_ADMIT.number(),
            1,
            at(),
            payload,
        )
    }

    #[test]
    fn a_source_says_where_it_lives_and_which_version_was_admitted() {
        // **Fix and copy**: the version is what makes a later change somebody else's business
        // rather than a reinterpretation of what has already been published.
        let source = born(&admitted(&[])).expect("complete");
        assert_eq!(source.name, "OpenID Connect");
        assert_eq!(source.version, "1.0 errata 2");
        assert_eq!(source.by, almena());
        assert!(source.admits(at()));
    }

    #[test]
    fn a_source_with_no_version_is_a_live_dependency_and_is_refused() {
        // Without one there is nothing to fix, and an external change would reinterpret what has
        // already been published — which is what `SPECS.md §4.3` forbids.
        for missing in [field::NAME, field::AT, field::VERSION, field::BY] {
            let mut act = admitted(&[]);
            act.payload.remove(&missing);
            assert_eq!(born(&act), Err(Refused::Malformed), "field {missing}");
        }
        assert_eq!(
            born(&admitted(&[(
                field::VERSION,
                Value::Text("   ".to_owned())
            )])),
            Err(Refused::Malformed),
            "and a version that says nothing is no version"
        );
    }

    #[test]
    fn deprecating_reaches_only_what_has_not_been_published_yet() {
        // Attributes already published copied their definitions and are self-sufficient. That is
        // the return on the fix-and-copy rule, and it is why this costs nobody anything.
        let source = born(&admitted(&[])).expect("complete");
        let later = Epoch::new(at().number() + 500);
        let mut gone = admitted(&[]);
        gone.kind = Kind::SOURCE_DEPRECATE.number();
        gone.issued = later;

        let after = does(&gone, &source, Kind::SOURCE_DEPRECATE).expect("signed");
        assert!(
            after.admits(at()),
            "what was published before goes on standing"
        );
        assert!(!after.admits(later), "and nothing new comes from it");
    }

    #[test]
    fn a_deprecation_is_said_once_and_its_date_is_not_moved_afterwards() {
        let source = born(&admitted(&[])).expect("complete");
        let mut gone = admitted(&[]);
        gone.kind = Kind::SOURCE_DEPRECATE.number();
        let after = does(&gone, &source, Kind::SOURCE_DEPRECATE).expect("signed");
        assert_eq!(
            does(&gone, &after, Kind::SOURCE_DEPRECATE),
            Err(Refused::NotAuthorised)
        );
    }

    #[test]
    fn who_is_admitting_it_is_read_from_the_act_and_checked_elsewhere() {
        assert_eq!(admitting(&admitted(&[])), Some(almena()));
        let mut nameless = admitted(&[]);
        nameless.payload.remove(&field::BY);
        assert_eq!(admitting(&nameless), None);
    }
}
