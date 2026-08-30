//! The shape of what is issued and of what is asked for.
//!
//! # What the catalogue exists for
//!
//! Not so that two systems understand each other — for that a schema would do. It exists so that
//! **asking for more than you need is visible** (`SPECS.md §9.4`). The holder does not choose the
//! template: the issuer decides what is issued and the verifier decides what is asked. Their power
//! is to accept or refuse, and to say no to what is optional. So the only thing left against excess
//! is that **what is asked for is public, comparable and refusable** — and a template is the
//! reference the comparison is made against.
//!
//! That is also why there is no private template and no bilateral arrangement outside the
//! catalogue: it is what makes the catalogue complete rather than a sample.
//!
//! # A version is a hash, and that is what stops the past being reinterpreted
//!
//! A credential names **one version, addressed by its hash**. If a template could change underneath
//! a credential already issued, changing it would reinterpret the past, which `SPECS.md §4.3`
//! forbids. So every `publish` is one version, each one stays resolvable, and a new one never
//! reaches back.
//!
//! # Derivation is declared, never inferred
//!
//! *The certified employment one, plus two fields.* With that, comparing stops being counting
//! attributes and becomes **a diff against a baseline** — which is where the excess shows on its
//! own. And the baseline is the author's own declaration, so choosing a flattering one is itself
//! visible in the diff.
//!
//! # Value or predicate, per attribute and not per request
//!
//! An age-restricted site has no business knowing a name or a date of birth; it needs an answer to
//! *are they old enough*. Configuring that in the template makes asking for anything else **a
//! decision visible in the catalogue**, which is the whole of the mechanism.
//!
//! **A derived attribute is not a zero-knowledge proof**, and the difference is not only
//! theoretical: it is revealed with a disclosure that is stable, so two verifiers who receive the
//! same credential can link that revelation. It minimises what is disclosed — which is the goal
//! here — and does not make a presentation unlinkable.

use std::collections::BTreeSet;

use almena_format::cbor::Value;
use almena_format::identifier::{Did, Name};
use almena_format::operation::Operation;
use almena_time::Epoch;

use crate::chain::Refused;
use crate::kind::Kind;

/// The most tags one template may carry.
///
/// **Three** (`SPECS.md §9.4`). It bounds the hoarding — putting on many so as to appear in every
/// comparison — without stopping a template from genuinely serving more than one end.
pub const TAGS_AT_MOST: usize = 3;

/// Where each part of a template act sits.
pub mod field {
    /// Whether it is a credential's shape or a request's.
    pub const KIND: u64 = 1;
    /// What it is called, for a person reading the catalogue.
    ///
    /// **Even**: a reader that passes over it holds the shape and not the title, which is a poorer
    /// catalogue entry and never a wrong claim about what is being asked for.
    pub const CALLED: u64 = 2;
    /// The attributes, each with how it is asked for.
    pub const ATTRIBUTES: u64 = 3;
    /// The template this one declares itself derived from.
    pub const DERIVES: u64 = 5;
    /// What it is for, from the closed list.
    pub const TAGS: u64 = 7;
    /// Who is publishing it.
    pub const BY: u64 = 9;
}

/// What a template is the shape of.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    /// What an issuer puts in a credential.
    Credential,
    /// What a verifier asks for.
    Request,
}

impl Shape {
    /// The shape a number names, if it is one this build knows.
    #[must_use]
    pub const fn of(number: u64) -> Option<Self> {
        match number {
            1 => Some(Self::Credential),
            2 => Some(Self::Request),
            _ => None,
        }
    }

    /// The number it travels as.
    #[must_use]
    pub const fn number(self) -> u64 {
        match self {
            Self::Credential => 1,
            Self::Request => 2,
        }
    }
}

/// How one attribute is asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum How {
    /// The value itself — *give me the date of birth*.
    Value,
    /// An answer about it — *show me they are old enough*.
    ///
    /// **Only where the attribute says it can answer one.** Asking a plain date for a predicate
    /// would be asking for something nobody undertook to be able to give.
    Predicate,
}

impl How {
    /// The way a number names, if it is one this build knows.
    #[must_use]
    pub const fn of(number: u64) -> Option<Self> {
        match number {
            1 => Some(Self::Value),
            2 => Some(Self::Predicate),
            _ => None,
        }
    }

    /// The number it travels as.
    #[must_use]
    pub const fn number(self) -> u64 {
        match self {
            Self::Value => 1,
            Self::Predicate => 2,
        }
    }
}

/// One attribute, as a template asks for it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Asked {
    /// Which attribute, by the name of the object that published it.
    pub attribute: Name,
    /// Whether the value is asked for or an answer about it.
    pub how: How,
    /// Whether refusing it means refusing the whole request.
    ///
    /// **Optional is what a holder may say no to** (`SPECS.md §9.2`), and a template where
    /// everything is required is one where the only choice is all or nothing. Saying which is
    /// which is the author's, and the catalogue is where it is read.
    pub required: bool,
}

/// One version of a template.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version {
    /// The act that published it, which is what a credential names.
    ///
    /// **Addressed by hash** (`SPECS.md §9.4`): a credential naming a version by anything a later
    /// act could change would be one whose meaning moves under it.
    pub called: Name,
    /// The epoch it was published in.
    pub at: Epoch,
    /// What it asks for.
    pub asks: Vec<Asked>,
    /// What it declares itself derived from, if anything.
    pub derives: Option<Name>,
    /// What it is for.
    pub tags: BTreeSet<Name>,
}

/// A template, as its chain says it stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Template {
    /// What it is the shape of.
    pub shape: Shape,
    /// Who publishes it.
    pub by: Did,
    /// Every version, oldest first. **All of them stay resolvable**, because credentials name them.
    pub versions: Vec<Version>,
    /// The epoch it was marked obsolete, if it was.
    pub deprecated: Option<Epoch>,
}

impl Template {
    /// The version in force, which is the last one published.
    #[must_use]
    pub fn latest(&self) -> Option<&Version> {
        self.versions.last()
    }

    /// Whether issuing or asking with it still passes without a warning.
    ///
    /// **Deprecation is not retroactive** (`SPECS.md §4.3`, `§9.4`): what has been issued goes on
    /// being valid, and what changes is that issuing with it warns.
    #[must_use]
    pub fn current(&self, at: Epoch) -> bool {
        self.deprecated
            .is_none_or(|since| at.number() < since.number())
    }
}

/// Who is publishing a template, read from the act.
#[must_use]
pub fn publishing(operation: &Operation) -> Option<Did> {
    match operation.payload.get(&field::BY) {
        Some(Value::Text(by)) => Did::parse(by).ok(),
        _ => None,
    }
}

/// Every attribute an act names, and how it asks for each.
///
/// **Held to strictly ascending names**, for the reason map keys are: canonical order is part of
/// what was signed and what the version's hash is taken over, so two orders of one set of attributes
/// would be two versions of one template.
///
/// # Errors
///
/// [`Refused::Malformed`] for anything that is not this shape.
pub fn asks(operation: &Operation) -> Result<Vec<Asked>, Refused> {
    let Some(Value::Array(listed)) = operation.payload.get(&field::ATTRIBUTES) else {
        return Err(Refused::Malformed);
    };
    // **An empty template asks for nothing and is not a shape.** One would be a request nobody
    // could compare and an issuance of nothing at all.
    if listed.is_empty() {
        return Err(Refused::Malformed);
    }

    let mut asks: Vec<Asked> = Vec::with_capacity(listed.len());
    for one in listed {
        let Value::Array(one) = one else {
            return Err(Refused::Malformed);
        };
        let [
            Value::Text(attribute),
            Value::Uint(how),
            Value::Uint(required),
        ] = one.as_slice()
        else {
            return Err(Refused::Malformed);
        };
        let attribute = Did::parse(attribute)
            .map(|named| named.name().clone())
            .map_err(|_| Refused::Malformed)?;
        if *required > 1 {
            return Err(Refused::Malformed);
        }
        if asks
            .last()
            .is_some_and(|before| before.attribute.as_str() >= attribute.as_str())
        {
            return Err(Refused::Malformed);
        }
        asks.push(Asked {
            attribute,
            how: How::of(*how).ok_or(Refused::Malformed)?,
            required: *required == 1,
        });
    }
    Ok(asks)
}

/// Every tag an act carries.
///
/// # Errors
///
/// [`Refused::Malformed`] for more than [`TAGS_AT_MOST`], or anything that is not a list of names.
pub fn tags(operation: &Operation) -> Result<BTreeSet<Name>, Refused> {
    let Some(value) = operation.payload.get(&field::TAGS) else {
        // **Carrying none is allowed and it shows** (`SPECS.md §9.4`): with a closed list nobody
        // can fall outside the taxonomy, so a template with no tags is one that chose not to be
        // compared, which the catalogue displays as exactly that.
        return Ok(BTreeSet::new());
    };
    let Value::Array(listed) = value else {
        return Err(Refused::Malformed);
    };
    if listed.len() > TAGS_AT_MOST {
        return Err(Refused::Malformed);
    }
    listed
        .iter()
        .map(|one| match one {
            Value::Text(tag) => Did::parse(tag)
                .map(|named| named.name().clone())
                .map_err(|_| Refused::Malformed),
            _ => Err(Refused::Malformed),
        })
        .collect()
}

/// What a template declares itself derived from, if anything.
#[must_use]
pub fn derives(operation: &Operation) -> Option<Name> {
    match operation.payload.get(&field::DERIVES) {
        Some(Value::Text(from)) => Did::parse(from).map(|named| named.name().clone()).ok(),
        _ => None,
    }
}

/// The fields a template act may carry that this build has a meaning for.
#[must_use]
pub fn vocabulary() -> almena_format::field::Vocabulary<'static> {
    use almena_format::field::Field;
    const FIELDS: &[Field] = &[
        Field::new(field::KIND),
        Field::new(field::ATTRIBUTES),
        Field::new(field::DERIVES),
        Field::new(field::TAGS),
        Field::new(field::BY),
        Field::new(crate::resolution::FIELD),
    ];
    const CLOSED: &[(Field, &[Value])] =
        &[(Field::new(field::KIND), &[Value::Uint(1), Value::Uint(2)])];
    almena_format::field::Vocabulary::with_closed(FIELDS, CLOSED)
}

/// A template, as the act that published its first version made it.
///
/// # Errors
///
/// [`Refused::Malformed`] for an act that is not a shape: no kind this build knows, nothing asked
/// for, more tags than may be carried, or nobody publishing it.
pub fn born(operation: &Operation) -> Result<Template, Refused> {
    let shape = match operation.payload.get(&field::KIND) {
        Some(Value::Uint(number)) => Shape::of(*number).ok_or(Refused::Malformed)?,
        _ => return Err(Refused::Malformed),
    };
    let asks = asks(operation)?;
    let tags = tags(operation)?;
    // **Tags classify what a request is for**, so a credential's shape carrying them would be
    // classifying something that is not an interaction.
    if shape == Shape::Credential && !tags.is_empty() {
        return Err(Refused::Malformed);
    }

    Ok(Template {
        shape,
        by: publishing(operation).ok_or(Refused::Malformed)?,
        versions: vec![Version {
            called: operation.called(),
            at: operation.issued,
            asks,
            derives: derives(operation),
            tags,
        }],
        deprecated: None,
    })
}

/// What an act does to a template.
///
/// # Errors
///
/// [`Refused`].
pub fn does(operation: &Operation, template: &Template, kind: Kind) -> Result<Template, Refused> {
    let mut next = template.clone();
    match kind {
        Kind::TEMPLATE_PUBLISH => {
            // **One `publish` is one version, and every previous one stays resolvable** — a
            // credential names the version it was issued against, and a version that stopped
            // resolving would be a credential nobody can read any more.
            let tags = tags(operation)?;
            if next.shape == Shape::Credential && !tags.is_empty() {
                return Err(Refused::Malformed);
            }
            next.versions.push(Version {
                called: operation.called(),
                at: operation.issued,
                asks: asks(operation)?,
                derives: derives(operation),
                tags,
            });
        }
        Kind::TEMPLATE_DEPRECATE => {
            if next.deprecated.is_some() {
                return Err(Refused::NotAuthorised);
            }
            next.deprecated = Some(operation.issued);
        }
        _ => return Err(Refused::Malformed),
    }
    Ok(next)
}

/// What one version asks for that another does not.
///
/// **The diff against the baseline**, which is what turns comparing from counting attributes into
/// seeing the excess on its own (`SPECS.md §9.4`). Both directions, because a template that asks for
/// *less* than the one it derives from is worth seeing too — and because a diff that only ever grew
/// would be one somebody could make look small by declaring a generous baseline.
#[must_use]
pub fn beyond<'a>(version: &'a Version, baseline: &Version) -> Vec<&'a Asked> {
    version
        .asks
        .iter()
        .filter(|asked| {
            !baseline
                .asks
                .iter()
                .any(|before| before.attribute == asked.attribute && before.how == asked.how)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{How, Shape, TAGS_AT_MOST, asks, beyond, born, does, field, tags};
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

    fn named(seed: u8) -> Did {
        Did::new(Network::Development, Name::of(&[seed; 8]))
    }

    /// The attributes a template asks for, in the one order it may write them in.
    fn asking(which: &[(u8, How, bool)]) -> Value {
        let mut listed: Vec<(String, &How, &bool)> = which
            .iter()
            .map(|(seed, how, required)| (named(*seed).to_string(), how, required))
            .collect();
        listed.sort_by(|one, other| one.0.cmp(&other.0));
        Value::Array(
            listed
                .into_iter()
                .map(|(attribute, how, required)| {
                    Value::Array(vec![
                        Value::Text(attribute),
                        Value::Uint(how.number()),
                        Value::Uint(u64::from(*required)),
                    ])
                })
                .collect(),
        )
    }

    fn published(shape: Shape, extra: &[(u64, Value)]) -> Operation {
        let mut payload = BTreeMap::from([
            (field::KIND, Value::Uint(shape.number())),
            (
                field::ATTRIBUTES,
                asking(&[(1, How::Value, true), (2, How::Value, false)]),
            ),
            (field::BY, Value::Text(named(9).to_string())),
        ]);
        for (which, value) in extra {
            payload.insert(*which, value.clone());
        }
        create(
            Network::Development,
            Kind::TEMPLATE_PUBLISH.number(),
            1,
            at(),
            payload,
        )
    }

    #[test]
    fn a_template_says_what_it_asks_for_and_which_of_it_may_be_refused() {
        // **Optional is what a holder may say no to** (`SPECS.md §9.2`), and a template where
        // everything is required is one where the only choice is all or nothing.
        let template = born(&published(Shape::Request, &[])).expect("complete");
        let version = template.latest().expect("one version");
        assert_eq!(version.asks.len(), 2);
        assert!(version.asks.iter().any(|asked| asked.required));
        assert!(version.asks.iter().any(|asked| !asked.required));
        assert!(template.current(at()));
    }

    #[test]
    fn a_template_that_asks_for_nothing_is_not_a_shape() {
        // One would be a request nobody could compare and an issuance of nothing at all.
        assert_eq!(
            born(&published(
                Shape::Request,
                &[(field::ATTRIBUTES, Value::Array(Vec::new()))]
            )),
            Err(Refused::Malformed)
        );
        for missing in [field::KIND, field::ATTRIBUTES, field::BY] {
            let mut act = published(Shape::Request, &[]);
            act.payload.remove(&missing);
            assert_eq!(born(&act), Err(Refused::Malformed), "field {missing}");
        }
    }

    #[test]
    fn attributes_written_in_two_orders_would_be_two_versions_of_one_template() {
        // Canonical order is part of what was signed and of what the version's hash is taken over.
        let backwards = Value::Array(vec![
            Value::Array(vec![
                Value::Text(named(2).to_string()),
                Value::Uint(1),
                Value::Uint(1),
            ]),
            Value::Array(vec![
                Value::Text(named(1).to_string()),
                Value::Uint(1),
                Value::Uint(1),
            ]),
        ]);
        let mut act = published(Shape::Request, &[(field::ATTRIBUTES, backwards)]);
        // Whichever way round the names sort, one of the two orders is the wrong one.
        let sorted = named(1).to_string() < named(2).to_string();
        if sorted {
            assert_eq!(asks(&act), Err(Refused::Malformed));
        } else {
            assert!(asks(&act).is_ok());
        }
        act.payload.insert(
            field::ATTRIBUTES,
            asking(&[(1, How::Value, true), (2, How::Value, true)]),
        );
        assert!(asks(&act).is_ok(), "and in order it reads");
    }

    #[test]
    fn no_more_than_three_tags_and_a_credential_carries_none() {
        // Three bounds the hoarding — putting on many so as to appear in every comparison — without
        // stopping a template serving more than one end. And tags classify what a *request* is for,
        // so a credential's shape carrying them would classify something that is not an interaction.
        let three = Value::Array(
            (1..=TAGS_AT_MOST)
                .map(|seed| Value::Text(named(u8::try_from(seed).unwrap_or(1) + 40).to_string()))
                .collect(),
        );
        assert_eq!(
            tags(&published(Shape::Request, &[(field::TAGS, three.clone())]))
                .map(|held| held.len()),
            Ok(TAGS_AT_MOST)
        );

        let four = Value::Array(
            (1..=TAGS_AT_MOST + 1)
                .map(|seed| Value::Text(named(u8::try_from(seed).unwrap_or(1) + 40).to_string()))
                .collect(),
        );
        assert_eq!(
            tags(&published(Shape::Request, &[(field::TAGS, four)])),
            Err(Refused::Malformed)
        );
        assert_eq!(
            born(&published(Shape::Credential, &[(field::TAGS, three)])),
            Err(Refused::Malformed)
        );
    }

    #[test]
    fn carrying_no_tags_is_allowed_and_shows() {
        // With a closed list nobody can fall outside the taxonomy, so a template with none is one
        // that chose not to be compared — which the catalogue displays as exactly that.
        let template = born(&published(Shape::Request, &[])).expect("complete");
        assert!(template.latest().expect("one").tags.is_empty());
    }

    #[test]
    fn every_version_stays_resolvable_because_a_credential_names_one() {
        // **A version is a hash** (`SPECS.md §9.4`). If a template could change underneath a
        // credential already issued, changing it would reinterpret the past.
        let first = published(Shape::Request, &[]);
        let template = born(&first).expect("complete");

        let mut second = published(
            Shape::Request,
            &[(
                field::ATTRIBUTES,
                asking(&[
                    (1, How::Value, true),
                    (2, How::Value, false),
                    (3, How::Value, false),
                ]),
            )],
        );
        second.previous = Some(first.called());
        second.issued = Epoch::new(at().number() + 10);

        let after = does(&second, &template, Kind::TEMPLATE_PUBLISH).expect("published");
        assert_eq!(after.versions.len(), 2);
        assert_eq!(
            after.versions[0].called,
            first.called(),
            "the version a credential already names is still there"
        );
        assert_eq!(after.versions[0].asks.len(), 2);
        assert_eq!(after.latest().expect("two").asks.len(), 3);
    }

    #[test]
    fn deprecating_leaves_what_was_issued_alone() {
        let template = born(&published(Shape::Request, &[])).expect("complete");
        let later = Epoch::new(at().number() + 500);
        let mut gone = published(Shape::Request, &[]);
        gone.kind = Kind::TEMPLATE_DEPRECATE.number();
        gone.issued = later;

        let after = does(&gone, &template, Kind::TEMPLATE_DEPRECATE).expect("signed");
        assert!(after.current(at()));
        assert!(!after.current(later));
        assert_eq!(
            does(&gone, &after, Kind::TEMPLATE_DEPRECATE),
            Err(Refused::NotAuthorised)
        );
    }

    #[test]
    fn the_diff_against_the_baseline_is_where_the_excess_shows_on_its_own() {
        // **Comparing stops being counting attributes.** And it runs both ways, because a template
        // asking for *less* than its baseline is worth seeing too — and because a diff that only
        // grew would be one somebody could make look small by declaring a generous baseline.
        let baseline = born(&published(Shape::Request, &[])).expect("complete");
        let derived = born(&published(
            Shape::Request,
            &[
                (
                    field::ATTRIBUTES,
                    asking(&[
                        (1, How::Value, true),
                        (2, How::Value, false),
                        (7, How::Value, true),
                    ]),
                ),
                (field::DERIVES, Value::Text(named(50).to_string())),
            ],
        ))
        .expect("complete");

        let one = baseline.latest().expect("one");
        let other = derived.latest().expect("one");
        let more = beyond(other, one);
        assert_eq!(more.len(), 1, "the two fields it added");
        assert_eq!(more[0].attribute, *named(7).name());
        assert!(beyond(one, other).is_empty(), "and nothing the other way");
        assert_eq!(other.derives, Some(named(50).name().clone()));
    }

    #[test]
    fn asking_for_an_answer_rather_than_a_value_is_a_different_thing_in_the_diff() {
        // A site with an age restriction has no business knowing a date of birth. Asking for the
        // value where the baseline asked for the answer is exactly what the diff must show.
        let baseline = born(&published(
            Shape::Request,
            &[(field::ATTRIBUTES, asking(&[(1, How::Predicate, true)]))],
        ))
        .expect("complete");
        let greedy = born(&published(
            Shape::Request,
            &[(field::ATTRIBUTES, asking(&[(1, How::Value, true)]))],
        ))
        .expect("complete");

        let more = beyond(
            greedy.latest().expect("one"),
            baseline.latest().expect("one"),
        );
        assert_eq!(more.len(), 1, "the same attribute, asked for differently");
        assert_eq!(more[0].how, How::Value);
    }

    #[test]
    fn a_way_of_asking_this_build_does_not_know_is_refused() {
        let odd = Value::Array(vec![Value::Array(vec![
            Value::Text(named(1).to_string()),
            Value::Uint(9),
            Value::Uint(1),
        ])]);
        assert_eq!(
            asks(&published(Shape::Request, &[(field::ATTRIBUTES, odd)])),
            Err(Refused::Malformed)
        );
        assert_eq!(How::of(9), None);
        assert_eq!(Shape::of(9), None);
        // **Read into the closed vocabulary and not carried as a number**, so that nothing
        // downstream has to remember to check it a second time.
        assert_eq!(How::of(How::Value.number()), Some(How::Value));
    }
}
