//! Asking for something: the request, and why anything beyond the template is malformed.
//!
//! # The template does not replace the standard, it fixes it
//!
//! `SPECS.md §9.2`. The request is expressed in the adopted standard — a DCQL query over
//! OpenID4VP — and **references by hash the template that authorises it**. What the template adds is
//! a ceiling: a request that asks for something the template does not, or asks for a value where
//! the template says an answer will do, or makes required what the template made optional, is
//! **malformed and refused** rather than shown to somebody as a choice.
//!
//! That is the load-bearing sentence of the whole catalogue. Excess is not stopped at the moment of
//! asking — it is stopped at the moment of *publishing*, where it is visible and comparable
//! (`SPECS.md §9.4`). What happens at the moment of asking is only that the request is held to what
//! was published.
//!
//! # And a purpose per attribute, or there is nothing to consent to
//!
//! `SPECS.md §9.2` requires it, and the holder signs it inside the presentation. A request that
//! asked for a date of birth without saying what for would be one somebody could only accept or
//! refuse whole, which is not consent to anything in particular.

use std::collections::BTreeMap;

use almena_format::identifier::Name;
use almena_store::template::{How, Version};

/// Which draft of the request format this build writes, fixed and copied rather than tracked.
///
/// The same discipline the attribute core follows: pinned, so that a request composed today goes on
/// meaning what it meant, and moving it is a decision somebody takes.
pub const QUERY_FORMAT: &str = "dcql";

/// What a credential is written in, as the query names it.
pub const CREDENTIAL_FORMAT: &str = almena_credential::MEDIA_TYPE;

/// One thing a verifier asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wanted {
    /// Which attribute, by the name of the object that published it.
    pub attribute: Name,
    /// Whether the value is asked for or an answer about it.
    pub how: How,
    /// Whether refusing it means refusing the whole request.
    ///
    /// **What a holder may say no to** (`SPECS.md §9.2`). Without optional attributes that a flow
    /// survives, selective disclosure is worth nothing in practice.
    pub required: bool,
    /// Whether it has to come from a credential rather than being typed in.
    ///
    /// **Distinguished in the request and on the screen** (`SPECS.md §9.2`): a verifier may insist
    /// that a claim be one an issuer signed, and a holder must be able to see which is which.
    pub from_credential: bool,
    /// What it is being asked for, in the verifier's own words.
    pub purpose: String,
}

/// One request, before it is written out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    /// The request template this is made under, by the hash of the act that published it.
    ///
    /// **What authorises it, and what it may not exceed** (`SPECS.md §9.2`, `§9.4`). It is a
    /// request's shape and not a credential's: the two are different objects with different
    /// purposes, and a template that was both would be a shape nobody could compare against
    /// anything.
    pub template: Name,
    /// The credential shapes this verifier takes the data from, by template version hash.
    ///
    /// **Empty is *any*.** A verifier that does not restrict which credential a claim comes from is
    /// taking any issuer's word for it, which is its own decision (`SPECS.md §12.2`).
    pub accepts: Vec<Name>,
    /// The nonce, which is what stops a presentation being replayed.
    pub nonce: String,
    /// Who the presentation is for, which is this verifier.
    pub audience: String,
    /// What is being asked for.
    pub wants: Vec<Wanted>,
}

/// Why a request is not one the template authorises.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Excess {
    /// It asks for nothing, which is not a request.
    Empty,
    /// It names a version other than the one it was checked against.
    AnotherTemplate,
    /// It asks for an attribute the template does not.
    NotInTemplate(Name),
    /// It asks for the value where the template says an answer about it will do.
    ///
    /// **The direction matters and only one of them is excess.** Asking for an answer where the
    /// template allows the value is asking for less, and less is always allowed.
    MoreThanNeeded(Name),
    /// It makes required what the template made optional.
    MoreRequired(Name),
    /// It asks for something without saying what for.
    NoPurpose(Name),
    /// It asks for one attribute twice.
    Twice(Name),
}

/// Whether the request is one the template authorises.
///
/// # Errors
///
/// [`Excess`], naming the attribute that put it over — because *this request asks for more than it
/// may* is not something to tell a person without saying which part.
pub fn holds_up(request: &Request, version: &Version) -> Result<(), Excess> {
    if request.wants.is_empty() {
        return Err(Excess::Empty);
    }
    if version.called != request.template {
        return Err(Excess::AnotherTemplate);
    }

    let mut seen: BTreeMap<&Name, ()> = BTreeMap::new();
    for wanted in &request.wants {
        if seen.insert(&wanted.attribute, ()).is_some() {
            return Err(Excess::Twice(wanted.attribute.clone()));
        }
        if wanted.purpose.trim().is_empty() {
            return Err(Excess::NoPurpose(wanted.attribute.clone()));
        }
        let asked = version
            .asks
            .iter()
            .find(|asked| asked.attribute == wanted.attribute)
            .ok_or_else(|| Excess::NotInTemplate(wanted.attribute.clone()))?;

        // **Asking for less is always allowed.** A verifier that asks whether somebody is old
        // enough where the template lets it ask for the date of birth has asked for less, and
        // nothing about the catalogue exists to stop that.
        if asked.how == How::Predicate && wanted.how == How::Value {
            return Err(Excess::MoreThanNeeded(wanted.attribute.clone()));
        }
        if wanted.required && !asked.required {
            return Err(Excess::MoreRequired(wanted.attribute.clone()));
        }
    }
    Ok(())
}

/// The request as the standard writes it, with what authorises it inside.
///
/// **The template's hash travels in the query**, so that whoever receives it can resolve the
/// template itself and hold the request to it — rather than taking the verifier's word that what it
/// is asking for is what it published.
#[must_use]
pub fn written(request: &Request) -> serde_json::Value {
    serde_json::json!({
        "format": QUERY_FORMAT,
        "nonce": request.nonce,
        "aud": request.audience,
        // What authorises the request, which is what whoever receives it holds the request to.
        "authorised_by": request.template.as_str(),
        "credentials": [{
            "id": "almena",
            "format": CREDENTIAL_FORMAT,
            // The credential shapes it will take the data from, by hash. Named by anything a later
            // act could change, they would be shapes whose meaning moves between the asking and the
            // answering.
            "meta": { "vct_values": request.accepts.iter().map(|one| one.as_str()).collect::<Vec<_>>() },
            "claims": request.wants.iter().map(|wanted| serde_json::json!({
                "path": [wanted.attribute.as_str()],
                "how": match wanted.how {
                    How::Value => "value",
                    How::Predicate => "predicate",
                },
                "required": wanted.required,
                "from_credential": wanted.from_credential,
                "purpose": wanted.purpose,
            })).collect::<Vec<_>>(),
        }],
    })
}

/// What each attribute is being asked for, which is what the holder signs inside the presentation.
#[must_use]
pub fn purposes(request: &Request) -> BTreeMap<String, String> {
    request
        .wants
        .iter()
        .map(|wanted| (wanted.attribute.as_str().to_owned(), wanted.purpose.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{Excess, Request, Wanted, holds_up, purposes, written};
    use almena_format::identifier::Name;
    use almena_store::template::{Asked, How, Version};
    use almena_time::Epoch;
    use std::collections::BTreeSet;

    fn named(seed: u8) -> Name {
        Name::of(&[seed; 8])
    }

    fn version(asks: &[(u8, How, bool)]) -> Version {
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

    fn wanting(wants: &[(u8, How, bool)]) -> Request {
        Request {
            template: named(99),
            accepts: vec![named(88)],
            nonce: "a-nonce".to_owned(),
            audience: "did:almena:dev:zAVerifier".to_owned(),
            wants: wants
                .iter()
                .map(|(seed, how, required)| Wanted {
                    attribute: named(*seed),
                    how: *how,
                    required: *required,
                    from_credential: true,
                    purpose: "to do the thing".to_owned(),
                })
                .collect(),
        }
    }

    #[test]
    fn a_request_within_its_template_holds_up() {
        let template = version(&[(1, How::Value, true), (2, How::Value, false)]);
        assert_eq!(
            holds_up(&wanting(&[(1, How::Value, true)]), &template),
            Ok(())
        );
        assert_eq!(
            holds_up(
                &wanting(&[(1, How::Value, true), (2, How::Value, false)]),
                &template
            ),
            Ok(())
        );
    }

    #[test]
    fn anything_beyond_the_template_is_malformed_and_says_which_part() {
        // **The load-bearing sentence of the catalogue** (`SPECS.md §9.2`, `§9.4`). Excess is
        // stopped where it is visible — at publication — and what happens here is that the request
        // is held to what was published.
        let template = version(&[(1, How::Value, true), (2, How::Value, false)]);
        assert_eq!(
            holds_up(&wanting(&[(9, How::Value, true)]), &template),
            Err(Excess::NotInTemplate(named(9)))
        );
        assert_eq!(
            holds_up(&wanting(&[(2, How::Value, true)]), &template),
            Err(Excess::MoreRequired(named(2))),
            "making required what the template made optional is asking for more"
        );
    }

    #[test]
    fn asking_for_less_than_the_template_allows_is_always_allowed() {
        // A verifier asking whether somebody is old enough where it could have asked for the date
        // of birth has asked for less, and nothing exists to stop that.
        let template = version(&[(1, How::Value, true)]);
        assert_eq!(
            holds_up(&wanting(&[(1, How::Predicate, true)]), &template),
            Ok(())
        );

        let predicate = version(&[(1, How::Predicate, true)]);
        assert_eq!(
            holds_up(&wanting(&[(1, How::Value, true)]), &predicate),
            Err(Excess::MoreThanNeeded(named(1))),
            "and the other direction is exactly the excess the template is a ceiling on"
        );
    }

    #[test]
    fn a_request_with_no_purpose_is_one_nobody_can_consent_to_in_particular() {
        let template = version(&[(1, How::Value, true)]);
        let mut silent = wanting(&[(1, How::Value, true)]);
        silent.wants[0].purpose = "   ".to_owned();
        assert_eq!(
            holds_up(&silent, &template),
            Err(Excess::NoPurpose(named(1)))
        );
    }

    #[test]
    fn a_request_against_another_version_is_not_this_one_authorised() {
        let template = version(&[(1, How::Value, true)]);
        let mut elsewhere = wanting(&[(1, How::Value, true)]);
        elsewhere.template = named(7);
        assert_eq!(
            holds_up(&elsewhere, &template),
            Err(Excess::AnotherTemplate)
        );
        assert_eq!(
            holds_up(&wanting(&[]), &template),
            Err(Excess::Empty),
            "and a request that asks for nothing is not one"
        );
    }

    #[test]
    fn one_attribute_asked_for_twice_is_two_answers_to_one_question() {
        let template = version(&[(1, How::Value, true)]);
        let mut twice = wanting(&[(1, How::Value, true)]);
        twice.wants.push(twice.wants[0].clone());
        assert_eq!(holds_up(&twice, &template), Err(Excess::Twice(named(1))));
    }

    #[test]
    fn the_template_travels_by_hash_inside_the_query() {
        // So that whoever receives it resolves the template itself and holds the request to it,
        // rather than taking the verifier's word for what it published.
        let request = wanting(&[(1, How::Value, true)]);
        let query = written(&request);
        assert_eq!(query["format"], "dcql");
        assert_eq!(query["authorised_by"], named(99).as_str());
        assert_eq!(
            query["credentials"][0]["meta"]["vct_values"][0],
            named(88).as_str(),
            "and the credential shapes it takes are a separate list"
        );
        assert_eq!(
            query["credentials"][0]["claims"][0]["purpose"],
            "to do the thing"
        );
        assert_eq!(purposes(&request)[named(1).as_str()], "to do the thing");
    }
}
