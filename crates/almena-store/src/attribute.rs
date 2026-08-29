//! One piece of data a credential can carry, with its definition copied in.
//!
//! # An attribute is not invented here, and it is not fetched either
//!
//! It comes from a public schema (`SPECS.md §9.4`), and the register **fixes the version and copies
//! the definition in** — the external reference stays as provenance and nothing is ever resolved
//! live. An external schema changes without warning, is not signed, is not in the record, and
//! cannot be fetched without going out to a host nobody chose; depending on one would let somebody
//! else's edit reinterpret credentials already issued, which `SPECS.md §4.3` forbids.
//!
//! # The labels a person reads live here, and that is deliberate
//!
//! Not in the template (`SPECS.md §9.4`). If they lived there, the same piece of data could read
//! differently in two templates — and that would break exactly the comparison the catalogue exists
//! to make possible.
//!
//! **English at least**, because it is the fallback (`SPECS.md §13.9`), and whatever else the author
//! contributes. Whatever is missing, the attribute stays usable: what is not translated is **shown
//! marked** on the consent screen rather than disguised. The one exception is the core, which Almena
//! publishes in every language the platform ships in, because Almena is the one maintaining it —
//! and it is what keeps the *untranslated* mark rare enough to still be read.
//!
//! # Adding a language later does not change what anything means
//!
//! `translate` is one more act on the attribute's chain and **does not change its identifier**, so
//! templates referencing it and credentials already issued go on standing. Translating adds how
//! something reads, never what it means.

use std::collections::{BTreeMap, BTreeSet};

use almena_format::cbor::Value;
use almena_format::identifier::{Did, Name};
use almena_format::operation::Operation;
use almena_time::Epoch;

use crate::chain::Refused;
use crate::kind::Kind;

/// The language every attribute has to be readable in.
///
/// **English, because it is the fallback** (`SPECS.md §13.9`). It is a floor and not a ceiling: an
/// attribute may carry any number of languages, and may not carry fewer than this.
pub const AT_LEAST: &str = "en";

/// Where each part of an attribute act sits.
pub mod field {
    /// The claim name it resolves to, which is what a credential carries.
    pub const CLAIM: u64 = 1;
    /// What it means exactly, in each language it is written in.
    ///
    /// **Even**: a reader that passes over it holds the attribute and its labels and not the
    /// sentence explaining it — a poorer consent screen, and never a wrong claim about the data.
    pub const MEANS: u64 = 2;
    /// Which kind of value it is.
    pub const TYPE: u64 = 3;
    /// Which source the definition was copied from.
    pub const SOURCE: u64 = 5;
    /// The definition itself, copied in.
    pub const DEFINITION: u64 = 7;
    /// Whether it may be asked for as a predicate rather than as a value.
    pub const PREDICATE: u64 = 9;
    /// The label a person reads, in each language it is written in.
    pub const LABELS: u64 = 11;
    /// Who is publishing it.
    pub const BY: u64 = 13;
}

/// What kind of value an attribute carries.
///
/// **Closed**, so a type this build does not know is refused rather than read as the nearest one —
/// reading a date as text would put something on a consent screen that nobody can compare.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Shape {
    /// Free text.
    Text,
    /// A date.
    Date,
    /// Yes or no — the shape a derived attribute like `age_over_18` takes.
    Boolean,
    /// A number.
    Number,
}

impl Shape {
    /// The shape a number names, if it is one this build knows.
    #[must_use]
    pub const fn of(number: u64) -> Option<Self> {
        match number {
            1 => Some(Self::Text),
            2 => Some(Self::Date),
            3 => Some(Self::Boolean),
            4 => Some(Self::Number),
            _ => None,
        }
    }

    /// The number it travels as.
    #[must_use]
    pub const fn number(self) -> u64 {
        match self {
            Self::Text => 1,
            Self::Date => 2,
            Self::Boolean => 3,
            Self::Number => 4,
        }
    }
}

/// Something written in every language it was written in.
pub type Written = BTreeMap<String, String>;

/// One piece of data, as its chain says it stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attribute {
    /// The claim name it resolves to.
    pub claim: String,
    /// What kind of value it carries.
    pub shape: Shape,
    /// Which source the definition was copied from.
    pub source: Name,
    /// The definition, copied in — which is what makes this self-sufficient.
    pub definition: String,
    /// Whether it may be asked for as a predicate rather than as a value.
    pub predicate: bool,
    /// The label a person reads, in each language it has been written in.
    pub labels: Written,
    /// What it means exactly, in each language it has been written in.
    pub means: Written,
    /// Who published it.
    pub by: Did,
    /// The epoch it stopped being one to use in new templates, if it has.
    pub deprecated: Option<Epoch>,
}

impl Attribute {
    /// Whether it may still be put in a new template at that moment.
    ///
    /// **Never retroactive** (`SPECS.md §4.3`): templates that already reference it and credentials
    /// already issued go on standing, and what changes is what may be built next.
    #[must_use]
    pub fn usable(&self, at: Epoch) -> bool {
        self.deprecated
            .is_none_or(|since| at.number() < since.number())
    }

    /// Every language it can be read in.
    #[must_use]
    pub fn languages(&self) -> BTreeSet<&str> {
        self.labels.keys().map(String::as_str).collect()
    }
}

/// Who is publishing an attribute, read from the act.
#[must_use]
pub fn publishing(operation: &Operation) -> Option<Did> {
    match operation.payload.get(&field::BY) {
        Some(Value::Text(by)) => Did::parse(by).ok(),
        _ => None,
    }
}

/// Which source an act says the definition was copied from.
#[must_use]
pub fn copied_from(operation: &Operation) -> Option<Name> {
    match operation.payload.get(&field::SOURCE) {
        Some(Value::Text(source)) => Did::parse(source).map(|source| source.name().clone()).ok(),
        _ => None,
    }
}

/// The fields an attribute act may carry that this build has a meaning for.
#[must_use]
pub fn vocabulary() -> almena_format::field::Vocabulary<'static> {
    use almena_format::field::Field;
    const FIELDS: &[Field] = &[
        Field::new(field::CLAIM),
        Field::new(field::TYPE),
        Field::new(field::SOURCE),
        Field::new(field::DEFINITION),
        Field::new(field::PREDICATE),
        Field::new(field::LABELS),
        Field::new(field::BY),
        Field::new(crate::resolution::FIELD),
    ];
    const CLOSED: &[(Field, &[Value])] = &[(
        Field::new(field::TYPE),
        &[
            Value::Uint(1),
            Value::Uint(2),
            Value::Uint(3),
            Value::Uint(4),
        ],
    )];
    almena_format::field::Vocabulary::with_closed(FIELDS, CLOSED)
}

/// An attribute, as the act that published it made it.
///
/// # Errors
///
/// [`Refused::Malformed`] for an act missing anything an attribute is — a claim name, a shape this
/// build knows, the source it came from, the definition copied in, a label in English, or who
/// published it.
pub fn born(operation: &Operation) -> Result<Attribute, Refused> {
    let shape = match operation.payload.get(&field::TYPE) {
        Some(Value::Uint(number)) => Shape::of(*number).ok_or(Refused::Malformed)?,
        _ => return Err(Refused::Malformed),
    };
    let labels = written(operation, field::LABELS)?;
    // **English at least**, because it is the fallback and because an attribute nobody can read is
    // one that cannot appear on a consent screen at all (`SPECS.md §9.4`, `§13.9`).
    if !labels.contains_key(AT_LEAST) {
        return Err(Refused::Malformed);
    }

    Ok(Attribute {
        claim: text(operation, field::CLAIM)?,
        shape,
        source: copied_from(operation).ok_or(Refused::Malformed)?,
        // **Copied in, which is what makes this self-sufficient.** Without it the register would be
        // pointing at somebody else's document and calling it a definition.
        definition: text(operation, field::DEFINITION)?,
        predicate: matches!(
            operation.payload.get(&field::PREDICATE),
            Some(Value::Uint(1))
        ),
        labels,
        means: written(operation, field::MEANS).unwrap_or_default(),
        by: publishing(operation).ok_or(Refused::Malformed)?,
        deprecated: None,
    })
}

/// What an act does to an attribute.
///
/// # Errors
///
/// [`Refused`].
pub fn does(
    operation: &Operation,
    attribute: &Attribute,
    kind: Kind,
) -> Result<Attribute, Refused> {
    let mut next = attribute.clone();
    match kind {
        Kind::ATTRIBUTE_TRANSLATE => {
            // **Adds how it reads, never what it means.** So the labels grow and everything else
            // is left exactly as it was — an act that could change the shape or the definition
            // under a template would be one that reinterprets credentials already issued.
            let more = written(operation, field::LABELS)?;
            if more.is_empty() {
                return Err(Refused::Malformed);
            }
            next.labels.extend(more);
            next.means
                .extend(written(operation, field::MEANS).unwrap_or_default());
        }
        Kind::ATTRIBUTE_DEPRECATE => {
            if next.deprecated.is_some() {
                return Err(Refused::NotAuthorised);
            }
            next.deprecated = Some(operation.issued);
        }
        _ => return Err(Refused::Malformed),
    }
    Ok(next)
}

/// Something written, in the one order it may be written in.
#[must_use]
pub fn carried(written: &Written) -> Value {
    Value::Array(
        written
            .iter()
            .map(|(tag, text)| {
                Value::Array(vec![Value::Text(tag.clone()), Value::Text(text.clone())])
            })
            .collect(),
    )
}

/// Something written, read from that field.
///
/// Held to strictly ascending language tags, for the reason map keys are: canonical order is part of
/// what was signed, and two orders of one set of labels would be two byte strings saying one thing.
///
/// **One reader for everything the register writes in more than one language**, so that a label, a
/// tag's name and a reason are all held to the same shape — three readers would be three chances
/// for one of them to accept something the others do not.
pub fn written_at(operation: &Operation, at: u64) -> Result<Written, Refused> {
    written(operation, at)
}

/// One text field, which has to be there and has to say something. Shared for the same reason.
///
/// # Errors
///
/// [`Refused::Malformed`] where it is missing or says nothing.
pub fn text_at(operation: &Operation, at: u64) -> Result<String, Refused> {
    text(operation, at)
}

/// Something written, read from that field.
fn written(operation: &Operation, at: u64) -> Result<Written, Refused> {
    let Some(Value::Array(pairs)) = operation.payload.get(&at) else {
        return Err(Refused::Malformed);
    };
    let mut said = Written::new();
    let mut last: Option<&str> = None;
    for pair in pairs {
        let Value::Array(pair) = pair else {
            return Err(Refused::Malformed);
        };
        let [Value::Text(tag), Value::Text(text)] = pair.as_slice() else {
            return Err(Refused::Malformed);
        };
        if tag.is_empty() || text.trim().is_empty() {
            return Err(Refused::Malformed);
        }
        if last.is_some_and(|before| before >= tag.as_str()) {
            return Err(Refused::Malformed);
        }
        last = Some(tag);
        said.insert(tag.clone(), text.clone());
    }
    Ok(said)
}

/// One text field, which has to be there and has to say something.
fn text(operation: &Operation, at: u64) -> Result<String, Refused> {
    match operation.payload.get(&at) {
        Some(Value::Text(text)) if !text.trim().is_empty() => Ok(text.clone()),
        _ => Err(Refused::Malformed),
    }
}

#[cfg(test)]
mod tests {
    use super::{AT_LEAST, Shape, Written, born, carried, does, field};
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

    fn who(seed: u8) -> Did {
        Did::new(Network::Development, Name::of(&[seed; 8]))
    }

    /// Labels, in the languages given.
    fn labels(languages: &[&str]) -> Value {
        carried(
            &languages
                .iter()
                .map(|tag| ((*tag).to_owned(), format!("date of birth, in {tag}")))
                .collect::<Written>(),
        )
    }

    fn published(extra: &[(u64, Value)]) -> Operation {
        let mut payload = BTreeMap::from([
            (field::CLAIM, Value::Text("birthdate".to_owned())),
            (field::TYPE, Value::Uint(Shape::Date.number())),
            (field::SOURCE, Value::Text(who(9).to_string())),
            (
                field::DEFINITION,
                Value::Text("End-User's birthday, represented as an ISO 8601 date.".to_owned()),
            ),
            (field::LABELS, labels(&["en", "es"])),
            (field::BY, Value::Text(who(1).to_string())),
        ]);
        for (which, value) in extra {
            payload.insert(*which, value.clone());
        }
        create(
            Network::Development,
            Kind::ATTRIBUTE_PUBLISH.number(),
            1,
            at(),
            payload,
        )
    }

    #[test]
    fn the_definition_is_copied_in_so_that_nothing_has_to_be_fetched_later() {
        // **Fix and copy, never depend live** (`SPECS.md §9.4`). An external schema changes without
        // warning, is not signed and is not in the record; depending on one would let somebody
        // else's edit reinterpret credentials already issued.
        let attribute = born(&published(&[])).expect("complete");
        assert_eq!(attribute.claim, "birthdate");
        assert_eq!(attribute.shape, Shape::Date);
        assert_eq!(attribute.source, *who(9).name());
        assert!(attribute.definition.contains("ISO 8601"));
        assert!(attribute.usable(at()));
    }

    #[test]
    fn an_attribute_with_no_definition_would_be_a_pointer_at_somebody_else_s_document() {
        for missing in [
            field::CLAIM,
            field::TYPE,
            field::SOURCE,
            field::DEFINITION,
            field::LABELS,
            field::BY,
        ] {
            let mut act = published(&[]);
            act.payload.remove(&missing);
            assert_eq!(born(&act), Err(Refused::Malformed), "field {missing}");
        }
    }

    #[test]
    fn an_attribute_nobody_can_read_cannot_go_on_a_consent_screen_at_all() {
        // **English at least**, because it is the fallback (`SPECS.md §13.9`). Everything else is
        // the author's to contribute, and what is missing is shown marked rather than disguised.
        assert_eq!(
            born(&published(&[(field::LABELS, labels(&["es"]))])),
            Err(Refused::Malformed)
        );
        let only_english = born(&published(&[(field::LABELS, labels(&[AT_LEAST]))]))
            .expect("the floor, and no more");
        assert_eq!(only_english.languages(), ["en"].into_iter().collect());
    }

    #[test]
    fn a_shape_this_build_does_not_know_is_refused_and_not_read_as_the_nearest_one() {
        // Reading a date as text would put something on a consent screen nobody can compare.
        assert_eq!(Shape::of(9), None);
        assert_eq!(
            born(&published(&[(field::TYPE, Value::Uint(9))])),
            Err(Refused::Malformed)
        );
    }

    #[test]
    fn translating_adds_how_it_reads_and_never_what_it_means() {
        // **And does not change its identifier**, which is what lets templates referencing it and
        // credentials already issued go on standing.
        let attribute = born(&published(&[(field::LABELS, labels(&["en"]))])).expect("complete");
        let mut translating = published(&[(field::LABELS, labels(&["fr"]))]);
        translating.kind = Kind::ATTRIBUTE_TRANSLATE.number();
        // A translation naming a different shape and a different definition changes neither.
        translating
            .payload
            .insert(field::TYPE, Value::Uint(Shape::Text.number()));
        translating
            .payload
            .insert(field::DEFINITION, Value::Text("something else".to_owned()));

        let after = does(&translating, &attribute, Kind::ATTRIBUTE_TRANSLATE).expect("signed");
        assert_eq!(after.languages(), ["en", "fr"].into_iter().collect());
        assert_eq!(after.shape, Shape::Date, "what it means did not move");
        assert_eq!(after.definition, attribute.definition);
    }

    #[test]
    fn labels_written_in_two_orders_would_be_two_sets_of_labels() {
        // Canonical order is part of what was signed, exactly as it is for map keys.
        let backwards = Value::Array(vec![
            Value::Array(vec![
                Value::Text("es".to_owned()),
                Value::Text("fecha de nacimiento".to_owned()),
            ]),
            Value::Array(vec![
                Value::Text("en".to_owned()),
                Value::Text("date of birth".to_owned()),
            ]),
        ]);
        assert_eq!(
            born(&published(&[(field::LABELS, backwards)])),
            Err(Refused::Malformed)
        );
    }

    #[test]
    fn a_label_with_nothing_behind_it_is_not_written_in_that_language() {
        let empty = Value::Array(vec![Value::Array(vec![
            Value::Text("en".to_owned()),
            Value::Text("  ".to_owned()),
        ])]);
        assert_eq!(
            born(&published(&[(field::LABELS, empty)])),
            Err(Refused::Malformed)
        );
    }

    #[test]
    fn whether_it_answers_a_predicate_is_the_attribute_s_own_business() {
        // A derived attribute like `age_over_18` is a boolean that answers a question, and a
        // template can only ask for one where the attribute says it can.
        assert!(!born(&published(&[])).expect("complete").predicate);
        let derived = born(&published(&[
            (field::TYPE, Value::Uint(Shape::Boolean.number())),
            (field::PREDICATE, Value::Uint(1)),
        ]))
        .expect("complete");
        assert!(derived.predicate);
    }

    #[test]
    fn deprecating_reaches_only_what_has_not_been_built_yet() {
        let attribute = born(&published(&[])).expect("complete");
        let later = Epoch::new(at().number() + 500);
        let mut gone = published(&[]);
        gone.kind = Kind::ATTRIBUTE_DEPRECATE.number();
        gone.issued = later;

        let after = does(&gone, &attribute, Kind::ATTRIBUTE_DEPRECATE).expect("signed");
        assert!(after.usable(at()), "what already references it stands");
        assert!(!after.usable(later));
        assert_eq!(
            does(&gone, &after, Kind::ATTRIBUTE_DEPRECATE),
            Err(Refused::NotAuthorised),
            "and it is said once"
        );
    }
}
