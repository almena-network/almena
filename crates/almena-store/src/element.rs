//! An issuer or a verifier: what hangs from an entity and does the work.
//!
//! **They are roles, not boxes** (`SPECS.md §2.3`). One entity can do both, and one element can be
//! declared as both — what separates them is what each is configured to do, not what class of thing
//! it is. So there is one object here with a role on it, rather than two objects that would have to
//! be kept in step for ever.
//!
//! # The link to the parent is proved in both directions by one act
//!
//! `SPECS.md §2.3` asks for a **verifiable bidirectional link**, and it takes no second act to get
//! one. The element names its parent in its own creation, which is the first direction. The second
//! is that the creation only enters the record if the **parent's owners signed it**, counted
//! against the parent's own set at that moment — so nobody can hang an element off an organisation
//! they do not govern, and the organisation does not have to acknowledge anything afterwards.
//!
//! An acknowledgement act would have been the obvious design and is the worse one: between the two
//! acts there is a window in which an element claims a parent that has not agreed, and everything
//! reading the record has to decide what to do about it.
//!
//! # The issuance key is authorised at the sealing threshold, and nowhere else
//!
//! Credentials are emitted one at a time by a key held by whoever operates the element, not by the
//! owners signing each one (`SPECS.md §4.11`). What the owners do is **authorise that key**, which
//! is a sealing act (`SPECS.md §8.2`) — so an element with no issuance key can be created by a
//! routine act and still issue nothing at all until the owners say so.

use std::collections::BTreeSet;

use almena_format::cbor::Value;
use almena_format::identifier::Did;
use almena_format::operation::Operation;
use almena_suite::ed25519;
use almena_time::Epoch;

use crate::chain::Refused;
use crate::entity::Class;
use crate::kind::Kind;

/// Where each part of an element's act sits.
///
/// **Odd is critical.** Two of these are even and say why: neither changes what the element is
/// allowed to do, only where somebody would go to find something.
pub mod field {
    /// The element's own key.
    pub const KEY: u64 = 1;
    /// Which node its status lists appear on first.
    ///
    /// **Even**: a reader that passes over it does not know where to look first and can still look
    /// anywhere, because a status list is replicated (`SPECS.md §10.2`).
    pub const PUBLISHES_AT: u64 = 2;
    /// The entity it hangs from.
    pub const OF: u64 = 3;
    /// Where it is called back.
    ///
    /// **Even**: an integration detail between an element and its own operator. A reader of the
    /// record that ignored it is missing nothing it needs to decide anything.
    pub const CALLBACK: u64 = 4;
    /// Whether it issues, verifies, or both.
    pub const ROLE: u64 = 5;
    /// The key it emits credentials with.
    pub const ISSUANCE: u64 = 7;
    /// Which templates it issues or requires.
    pub const SERVES: u64 = 9;
}

/// What an element does.
///
/// A pair of flags rather than a choice of one, because `SPECS.md §2.3` says these are roles and
/// one thing can hold both.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Role {
    /// It emits credentials.
    pub issues: bool,
    /// It asks for presentations.
    pub verifies: bool,
}

impl Role {
    /// How it travels: a flag each, so that a third role is one more bit and not a new vocabulary.
    const ISSUES: u64 = 1;
    /// The flag for asking.
    const VERIFIES: u64 = 2;

    /// The role that number is, if it is one at all.
    ///
    /// **Nought is refused.** An element that neither issues nor verifies is one nothing can be
    /// done with, and taking it would be storing a thing whose whole purpose was left blank.
    #[must_use]
    pub const fn of(number: u64) -> Option<Self> {
        let role = Self {
            issues: number & Self::ISSUES != 0,
            verifies: number & Self::VERIFIES != 0,
        };
        if !role.issues && !role.verifies {
            return None;
        }
        Some(role)
    }

    /// The number it travels as.
    #[must_use]
    pub const fn number(self) -> u64 {
        let mut number = 0;
        if self.issues {
            number |= Self::ISSUES;
        }
        if self.verifies {
            number |= Self::VERIFIES;
        }
        number
    }
}

/// An issuer or a verifier, as its chain says it stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Element {
    /// The entity it hangs from, whose owners created it.
    pub of: Did,
    /// What it does.
    pub role: Role,
    /// Its own key.
    pub key: [u8; ed25519::PUBLIC_KEY_WIDTH],
    /// The key it emits credentials with, once the owners have authorised one.
    ///
    /// **[`None`] is an element that cannot issue anything**, which is what one is until a sealing
    /// act says otherwise (`SPECS.md §4.11`, `§8.2`). It is a real state and the ordinary one for a
    /// verifier, which never has one.
    pub issuance: Option<Vec<u8>>,
    /// Which templates it issues or requires.
    ///
    /// Names, kept as they were written. What a template *is* arrives with the work that publishes
    /// them; what matters here is that the configuration is public, because it is what the
    /// catalogue's comparison reads (`SPECS.md §2.3`).
    pub serves: BTreeSet<String>,
    /// Where it is called back, when its operator has said.
    pub callback: Option<String>,
    /// Which node its status lists appear on first.
    pub publishes_at: Option<Did>,
    /// When it was closed, if it was.
    pub closed: Option<Epoch>,
}

/// Which class of the parent's threshold each act on an element is counted against.
///
/// **The parent's threshold, because an element has no owners of its own.** It is a thing an
/// organisation runs, and who may change it is the organisation's question.
#[must_use]
pub const fn class(kind: Kind) -> Option<Class> {
    Some(match kind {
        // Creating one and configuring it are reversible and hold no authority on their own: an
        // element with no issuance key issues nothing whatever it is configured to serve.
        Kind::ISSUER_CREATE | Kind::ISSUER_SET_CONFIG => Class::Routine,
        // **Authorising the key credentials are actually emitted with** (`SPECS.md §8.2`), which is
        // the act that turns a configuration into something that can sign in the world's face.
        Kind::ISSUER_SET_ISSUANCE_KEY => Class::Sealing,
        Kind::ISSUER_ROTATE_KEY | Kind::ISSUER_CLOSE => Class::Governance,
        _ => return None,
    })
}

/// The fields an element act may carry that this build has a meaning for.
#[must_use]
pub fn vocabulary() -> almena_format::field::Vocabulary<'static> {
    use almena_format::field::Field;
    const FIELDS: &[Field] = &[
        Field::new(field::KEY),
        Field::new(field::OF),
        Field::new(field::ROLE),
        Field::new(field::ISSUANCE),
        Field::new(field::SERVES),
        Field::new(crate::resolution::FIELD),
    ];
    almena_format::field::Vocabulary::of(FIELDS)
}

/// An element, as the act that created it made it.
///
/// # Errors
///
/// [`Refused::Malformed`] for an act that does not carry a parent, a key and a role it can read.
pub fn born(operation: &Operation) -> Result<Element, Refused> {
    let of = match operation.payload.get(&field::OF) {
        Some(Value::Text(text)) => Did::parse(text).map_err(|_| Refused::Malformed)?,
        _ => return Err(Refused::Malformed),
    };
    let role = match operation.payload.get(&field::ROLE) {
        Some(Value::Uint(number)) => Role::of(*number).ok_or(Refused::Malformed)?,
        _ => return Err(Refused::Malformed),
    };
    let Some(Value::Bytes(key)) = operation.payload.get(&field::KEY) else {
        return Err(Refused::Malformed);
    };

    Ok(Element {
        of,
        role,
        key: key.as_slice().try_into().map_err(|_| Refused::Malformed)?,
        issuance: None,
        serves: served(operation)?,
        callback: text(operation, field::CALLBACK),
        publishes_at: text(operation, field::PUBLISHES_AT).and_then(|text| Did::parse(&text).ok()),
        closed: None,
    })
}

/// What an act does to an element, once the parent's owners have been counted.
///
/// # Errors
///
/// [`Refused`].
pub fn does(operation: &Operation, element: &Element, kind: Kind) -> Result<Element, Refused> {
    let mut next = element.clone();
    match kind {
        Kind::ISSUER_SET_CONFIG => {
            next.serves = served(operation)?;
            // **Absent means unchanged, and empty means cleared.** Two different things, and a
            // reader that collapsed them would silently drop an operator's callback the first time
            // they changed what they serve.
            if operation.payload.contains_key(&field::CALLBACK) {
                next.callback = text(operation, field::CALLBACK);
            }
            if operation.payload.contains_key(&field::PUBLISHES_AT) {
                next.publishes_at =
                    text(operation, field::PUBLISHES_AT).and_then(|text| Did::parse(&text).ok());
            }
        }
        Kind::ISSUER_SET_ISSUANCE_KEY => {
            let Some(Value::Bytes(key)) = operation.payload.get(&field::ISSUANCE) else {
                return Err(Refused::Malformed);
            };
            // **A verifier has no issuance key**, and one carrying it would be an element the
            // record says can emit credentials while its own role says it cannot.
            if !next.role.issues {
                return Err(Refused::NotAuthorised);
            }
            next.issuance = Some(key.clone());
        }
        Kind::ISSUER_ROTATE_KEY => {
            let Some(Value::Bytes(key)) = operation.payload.get(&field::KEY) else {
                return Err(Refused::Malformed);
            };
            next.key = key.as_slice().try_into().map_err(|_| Refused::Malformed)?;
        }
        Kind::ISSUER_CLOSE => next.closed = Some(operation.issued),
        _ => return Err(Refused::Malformed),
    }
    Ok(next)
}

/// The templates an act names.
fn served(operation: &Operation) -> Result<BTreeSet<String>, Refused> {
    let Some(value) = operation.payload.get(&field::SERVES) else {
        return Ok(BTreeSet::new());
    };
    let Value::Array(listed) = value else {
        return Err(Refused::Malformed);
    };
    listed
        .iter()
        .map(|one| match one {
            Value::Text(name) => Ok(name.clone()),
            _ => Err(Refused::Malformed),
        })
        .collect()
}

/// One text field, when there is one.
fn text(operation: &Operation, at: u64) -> Option<String> {
    match operation.payload.get(&at) {
        Some(Value::Text(text)) if !text.is_empty() => Some(text.clone()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{Element, Role, born, class, does, field};
    use crate::chain::Refused;
    use crate::entity::Class;
    use crate::kind::Kind;
    use almena_format::cbor::Value;
    use almena_format::identifier::{Did, Name, Network};
    use almena_format::operation::{Operation, create};
    use almena_time::Epoch;
    use std::collections::{BTreeMap, BTreeSet};

    fn parent() -> Did {
        Did::new(Network::Development, Name::of(b"an entity"))
    }

    fn creation(role: u64, extra: &[(u64, Value)]) -> Operation {
        let mut payload = BTreeMap::from([
            (field::KEY, Value::Bytes(vec![7; 32])),
            (field::OF, Value::Text(parent().to_string())),
            (field::ROLE, Value::Uint(role)),
        ]);
        payload.extend(extra.iter().cloned());
        create(
            Network::Development,
            Kind::ISSUER_CREATE.number(),
            1,
            Epoch::GENESIS,
            payload,
        )
    }

    fn an_element(role: Role) -> Element {
        Element {
            of: parent(),
            role,
            key: [7; 32],
            issuance: None,
            serves: BTreeSet::new(),
            callback: Some("https://issuer.example/back".to_owned()),
            publishes_at: None,
            closed: None,
        }
    }

    fn act(kind: Kind, payload: BTreeMap<u64, Value>) -> Operation {
        Operation {
            object: Did::new(Network::Development, Name::of(b"an element")),
            previous: Some(Name::of(b"before")),
            kind: kind.number(),
            version: 1,
            issued: Epoch::GENESIS,
            payload,
            signatures: Vec::new(),
        }
    }

    #[test]
    fn an_element_names_the_entity_it_hangs_from() {
        // The first half of the bidirectional link. The second half is that this act only enters
        // the record if the parent's owners signed it, which is counted where entities are.
        let element = born(&creation(1, &[])).expect("a parent, a key and a role");
        assert_eq!(element.of, parent());
        assert!(element.role.issues && !element.role.verifies);
        assert_eq!(element.issuance, None, "and it can issue nothing yet");
    }

    #[test]
    fn one_element_can_be_both_because_these_are_roles_and_not_boxes() {
        let element = born(&creation(3, &[])).expect("both");
        assert!(element.role.issues && element.role.verifies);
        assert_eq!(Role::of(3).map(Role::number), Some(3));
    }

    #[test]
    fn an_element_that_neither_issues_nor_verifies_is_refused() {
        // Nothing can be done with one, and taking it would be storing a thing whose whole purpose
        // was left blank.
        assert_eq!(Role::of(0), None);
        assert_eq!(born(&creation(0, &[])), Err(Refused::Malformed));
    }

    #[test]
    fn authorising_the_key_credentials_are_emitted_with_is_a_sealing_act() {
        // Creating and configuring one hold no authority: an element issues nothing at all until
        // the owners say which key it emits with (`SPECS.md §4.11`, `§8.2`).
        assert_eq!(class(Kind::ISSUER_CREATE), Some(Class::Routine));
        assert_eq!(class(Kind::ISSUER_SET_CONFIG), Some(Class::Routine));
        assert_eq!(class(Kind::ISSUER_SET_ISSUANCE_KEY), Some(Class::Sealing));
        assert_eq!(class(Kind::ISSUER_CLOSE), Some(Class::Governance));
        assert_eq!(class(Kind::HOLDER_FREEZE), None);
    }

    #[test]
    fn a_verifier_is_never_given_a_key_to_emit_with() {
        // It would be an element the record says can sign credentials while its own role says it
        // cannot, which is a disagreement inside one object.
        let signed = act(
            Kind::ISSUER_SET_ISSUANCE_KEY,
            BTreeMap::from([(field::ISSUANCE, Value::Bytes(vec![3; 32]))]),
        );
        assert_eq!(
            does(
                &signed,
                &an_element(Role {
                    issues: false,
                    verifies: true
                }),
                Kind::ISSUER_SET_ISSUANCE_KEY
            ),
            Err(Refused::NotAuthorised)
        );
        assert!(
            does(
                &signed,
                &an_element(Role {
                    issues: true,
                    verifies: false
                }),
                Kind::ISSUER_SET_ISSUANCE_KEY
            )
            .is_ok()
        );
    }

    #[test]
    fn a_field_left_out_of_a_configuration_is_unchanged_and_an_empty_one_is_cleared() {
        // Two different things. Collapsed, an operator would silently lose their callback the first
        // time they changed what they serve.
        let element = an_element(Role {
            issues: true,
            verifies: false,
        });
        let untouched = does(
            &act(
                Kind::ISSUER_SET_CONFIG,
                BTreeMap::from([(
                    field::SERVES,
                    Value::Array(vec![Value::Text("zTemplate".to_owned())]),
                )]),
            ),
            &element,
            Kind::ISSUER_SET_CONFIG,
        )
        .expect("configured");
        assert_eq!(untouched.callback, element.callback);
        assert_eq!(untouched.serves, BTreeSet::from(["zTemplate".to_owned()]));

        let cleared = does(
            &act(
                Kind::ISSUER_SET_CONFIG,
                BTreeMap::from([(field::CALLBACK, Value::Text(String::new()))]),
            ),
            &element,
            Kind::ISSUER_SET_CONFIG,
        )
        .expect("configured");
        assert_eq!(cleared.callback, None);
    }
}
