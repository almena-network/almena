//! Issuing against a template, and revoking afterwards.
//!
//! # The issuer decides what goes in; the template decides what may
//!
//! `SPECS.md §9.4`. An issuer signs what it likes about somebody — but a credential names the
//! template version it was issued against, and a verifier holds it to that. So a credential whose
//! attributes are not the ones the template says is one nobody can ask for, which makes it a
//! mistake worth catching here rather than at the counter.
//!
//! # Issuance is not conditional on acceptance; delivery is
//!
//! `SPECS.md §9.5`. The issuer signs and sends; the holder accepts or refuses, and what it decides
//! is what enters **its wallet**, not what the issuer's own records say. Your university knows you
//! graduated whatever you accept. A credential signed and never collected costs almost nothing: it
//! is inert without the holder's key, it expires, and its whole cohort is thrown away.
//!
//! # And revoking has to be as cheap as issuing
//!
//! Publishing a new version of a status list is signed by the element's own key
//! (`SPECS.md §10.2`). An issuer that had to convene its owners to flip a bit is an issuer that
//! does not revoke at the speed a revocation is for.

use std::collections::BTreeMap;

use almena_credential::issue::{Issued, NotIssued, sign};
use almena_credential::{About, Method, Proof, Status};
use almena_format::cbor::Value;
use almena_format::identifier::{Did, Name, Network};
use almena_format::operation::{Operation, create};
use almena_status::list::{List, NoRandomness, somewhere};
use almena_store::kind::Kind;
use almena_store::status::field;
use almena_store::template::{How, Version};
use almena_suite::p256;
use almena_time::cohort::Cohort;
use almena_time::{Clock, Epoch};

/// What is being issued, before it is signed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issuing<'a> {
    /// The issuer element doing it.
    pub issuer: &'a Did,
    /// The template version it is issued against.
    pub template: &'a Version,
    /// The credential's own identifier, which travels as a disclosure and never in the clear.
    pub identifier: &'a str,
    /// What it says, by attribute.
    pub attributes: &'a BTreeMap<Name, serde_json::Value>,
    /// The key the holder generated for this credential, which is what binds it to them.
    pub holder: &'a p256::VerifyingKey,
    /// When it was issued and when it stops being valid.
    pub between: (Epoch, Epoch),
    /// Where its bit is, or that there is not one.
    pub status: Status,
}

/// Why a credential could not be issued against that template.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotAgainstIt {
    /// It carries an attribute the template does not ask for.
    NotInTemplate(Name),
    /// The template asks for something required that it does not carry.
    Missing(Name),
    /// The template asks for an answer about that attribute, and the value is not a yes or no.
    ///
    /// **A predicate is satisfied by a derived attribute the issuer computed at issuance**
    /// (`SPECS.md §9.1`, `§9.4`) — not by handing over the value and letting somebody work it out.
    NotAnAnswer(Name),
    /// The credential itself is not one: nothing in it, or already expired.
    NotACredential(NotIssued),
}

/// Sign a credential against a template version.
///
/// # Errors
///
/// [`NotAgainstIt`], naming the attribute that does not fit — because *this does not match the
/// template* is not something to hand back without saying which part.
pub fn issue(issuing: &Issuing<'_>, key: &p256::SigningKey) -> Result<Issued, NotAgainstIt> {
    for (attribute, value) in issuing.attributes {
        let asked = issuing
            .template
            .asks
            .iter()
            .find(|asked| &asked.attribute == attribute)
            .ok_or_else(|| NotAgainstIt::NotInTemplate(attribute.clone()))?;
        if asked.how == How::Predicate && !value.is_boolean() {
            return Err(NotAgainstIt::NotAnAnswer(attribute.clone()));
        }
    }
    for asked in &issuing.template.asks {
        // **Only the required ones.** A template's optional attributes are optional to the issuer
        // as well: what is not in a credential is what a holder was never able to show.
        if asked.required && !issuing.attributes.contains_key(&asked.attribute) {
            return Err(NotAgainstIt::Missing(asked.attribute.clone()));
        }
    }

    let named: BTreeMap<String, serde_json::Value> = issuing
        .attributes
        .iter()
        .map(|(attribute, value)| (attribute.as_str().to_owned(), value.clone()))
        .collect();
    sign(
        &About {
            issuer: issuing.issuer.to_string(),
            template: issuing.template.called.as_str().to_owned(),
            issued: issuing.between.0,
            expires: issuing.between.1,
            // One member each today. The fields exist so that a second one is an addition rather
            // than a migration, and so that a reader that meets one stops instead of assuming.
            proof: Proof::Disclosure,
            method: Method::Almena,
            status: issuing.status.clone(),
        },
        issuing.identifier,
        &named,
        issuing.holder,
        key,
    )
    .map_err(NotAgainstIt::NotACredential)
}

/// Where a credential's bit goes: which list, and a place in it drawn at random.
///
/// **The cohort comes from the expiry** (`SPECS.md §10.2`), so an issuer keeps one list per quarter
/// of expiries and throws each away whole when its window passes.
/// # Errors
///
/// [`NoRandomness`] when the operating system will not produce any. A counter would put back the
/// exact fact the randomness removes, so there is no falling back to one.
pub fn place(list: &Did, entries: u64) -> Result<Status, NoRandomness> {
    Ok(Status::Revocable {
        list: list.to_string(),
        index: somewhere(entries)?,
    })
}

/// Which cohort a credential expiring then belongs to.
#[must_use]
pub fn cohort(clock: &Clock, expires: Epoch) -> Option<Cohort> {
    Cohort::of(clock, expires)
}

/// The act that publishes a version of a status list, unsigned.
///
/// **Only the hash** (`SPECS.md §10.2`): the bytes are hosted on the network and addressed by it,
/// and putting sixteen kilobytes of mostly noughts into a log every node keeps for ever would be
/// saying with a file what a digest says.
///
/// Unsigned, because signing is the issuer's: what leaves here is what is about to happen.
#[must_use]
pub fn publishing(network: Network, list: &List, by: &Did, cohort: Cohort, at: Epoch) -> Operation {
    create(
        network,
        Kind::STATUS_LIST_PUBLISH_VERSION.number(),
        1,
        at,
        BTreeMap::from([
            (
                field::VERSION,
                Value::Bytes(list.version().bytes().to_vec()),
            ),
            (field::COHORT, Value::Text(cohort.written())),
            (field::BY, Value::Text(by.to_string())),
        ]),
    )
}

/// The act that publishes a later version on a list that already exists.
#[must_use]
pub fn republishing(list: &List, on: &Did, previous: Name, at: Epoch) -> Operation {
    Operation {
        object: on.clone(),
        previous: Some(previous),
        kind: Kind::STATUS_LIST_PUBLISH_VERSION.number(),
        version: 1,
        issued: at,
        payload: BTreeMap::from([(
            field::VERSION,
            Value::Bytes(list.version().bytes().to_vec()),
        )]),
        signatures: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::{Issuing, NotAgainstIt, issue, place, publishing};
    use almena_credential::Status;
    use almena_format::identifier::{Did, Name, Network};
    use almena_status::list::{AT_LEAST, List};
    use almena_store::status;
    use almena_store::template::{Asked, How, Version};
    use almena_suite::p256;
    use almena_time::Epoch;
    use almena_time::cohort::Cohort;
    use std::collections::{BTreeMap, BTreeSet};

    fn key(seed: u8) -> p256::SigningKey {
        p256::SigningKey::from_secret([seed; 32]).expect("a key")
    }

    fn named(seed: u8) -> Name {
        Name::of(&[seed; 8])
    }

    fn issuer() -> Did {
        Did::new(Network::Development, named(9))
    }

    fn template(asks: &[(u8, How, bool)]) -> Version {
        Version {
            called: named(99),
            at: Epoch::new(100),
            asks: asks
                .iter()
                .map(|(seed, how, required)| Asked {
                    attribute: named(*seed),
                    how: *how,
                    required: *required,
                })
                .collect(),
            derives: None,
            tags: BTreeSet::new(),
        }
    }

    fn issuing<'a>(
        template: &'a Version,
        attributes: &'a BTreeMap<Name, serde_json::Value>,
        holder: &'a p256::VerifyingKey,
        issuer: &'a Did,
    ) -> Issuing<'a> {
        Issuing {
            issuer,
            template,
            identifier: "credential-one",
            attributes,
            holder,
            between: (Epoch::new(100), Epoch::new(10_000)),
            status: Status::NotRevocable,
        }
    }

    #[test]
    fn a_credential_carries_what_its_template_asks_for_and_nothing_else() {
        let template = template(&[(1, How::Value, true), (2, How::Value, false)]);
        let holder = key(2).verifying_key();
        let who = issuer();

        let held = BTreeMap::from([(named(1), serde_json::json!("Ada"))]);
        assert!(
            issue(&issuing(&template, &held, &holder, &who), &key(1)).is_ok(),
            "the required one is enough: what is optional is optional to the issuer too"
        );

        let extra = BTreeMap::from([
            (named(1), serde_json::json!("Ada")),
            (named(7), serde_json::json!("something else")),
        ]);
        assert_eq!(
            issue(&issuing(&template, &extra, &holder, &who), &key(1)),
            Err(NotAgainstIt::NotInTemplate(named(7)))
        );

        let short = BTreeMap::from([(named(2), serde_json::json!("Lovelace"))]);
        assert_eq!(
            issue(&issuing(&template, &short, &holder, &who), &key(1)),
            Err(NotAgainstIt::Missing(named(1)))
        );
    }

    #[test]
    fn a_predicate_is_answered_and_never_handed_over_to_be_worked_out() {
        // **The issuer computes the derived attribute at issuance** (`SPECS.md §9.1`, `§9.4`).
        // Handing over the date of birth and letting the verifier do the arithmetic is exactly what
        // asking for a predicate exists to avoid.
        let template = template(&[(1, How::Predicate, true)]);
        let holder = key(2).verifying_key();
        let who = issuer();

        let date = BTreeMap::from([(named(1), serde_json::json!("1815-12-10"))]);
        assert_eq!(
            issue(&issuing(&template, &date, &holder, &who), &key(1)),
            Err(NotAgainstIt::NotAnAnswer(named(1)))
        );
        let answer = BTreeMap::from([(named(1), serde_json::json!(true))]);
        assert!(issue(&issuing(&template, &answer, &holder, &who), &key(1)).is_ok());
    }

    #[test]
    fn a_place_in_a_list_is_drawn_from_the_whole_of_it() {
        let list = Did::new(Network::Development, named(5));
        let Ok(Status::Revocable {
            list: named_list,
            index,
        }) = place(&list, AT_LEAST)
        else {
            panic!("a place in the list")
        };
        assert_eq!(named_list, list.to_string());
        assert!(index < AT_LEAST);
    }

    #[test]
    fn publishing_a_version_puts_the_hash_in_the_record_and_nothing_else() {
        let mut held = List::empty();
        held.revoke(11);
        let act = publishing(
            Network::Development,
            &held,
            &issuer(),
            Cohort {
                year: 2026,
                quarter: 3,
            },
            Epoch::new(200),
        );
        let read = status::born(&act).expect("a list");
        assert_eq!(
            read.latest().expect("one").hash,
            held.version().bytes().to_vec()
        );
        assert_eq!(read.cohort.written(), "2026-Q3");
        assert_eq!(read.by, issuer());
    }
}
