//! Verifying a presentation: everything checked, and what may be concluded when something is not.
//!
//! # Five questions, and one of them is not about the credential
//!
//! Whether the issuer signed it, whether the holder holds it, whether it was issued against the
//! template the request names, whether it has been revoked, and whether what was shown is what was
//! asked for. Four are answered from the presentation itself; **revocation depends on reaching
//! somewhere**, and that is why the answer has three shapes rather than two (`SPECS.md §17.12`).
//!
//! # The trust chain is behind an interface, and that is not decoration
//!
//! `SPECS.md §9.1` asks that interoperability with X.509 and state trust lists stay open. So what a
//! verifier brings is [`Resolved`] — the answers, whatever produced them — and what it accepts is a
//! list of identification methods **it chose**. This crate never resolves anything, and never
//! decides whose seal is worth something: that is the reader's own policy, and a library that had a
//! default for it would be making somebody's trust decisions under the name of one.

use almena_credential::Method;
use almena_credential::verify::{Facts, Fault, Missing, Outcome, Revocation, check};
use almena_format::identifier::Name;
use almena_suite::p256;
use almena_time::Epoch;

use crate::request::{Request, purposes};

/// What the record answered about the issuer, brought by whoever is verifying.
#[derive(Debug, Clone)]
pub struct Resolved {
    /// The key the record says the issuer emits with.
    ///
    /// **[`None`] is *nobody could be asked***, never *there is no such issuer*.
    pub issuance_key: Option<p256::VerifyingKey>,
    /// Whether the organisation behind it is closed.
    pub closed: bool,
}

/// What this verifier insists on, which is its own to decide.
#[derive(Debug, Clone)]
pub struct Policy<'a> {
    /// The identification methods it accepts. **Its own list**, never the credential's proposal.
    pub methods: &'a [Method],
    /// Whether a credential that says it cannot be revoked is refused (`SPECS.md §10.1`).
    pub revocable: bool,
    /// Whether credentials from organisations already closed are still taken (`SPECS.md §12.2`).
    pub closed_issuers: bool,
}

/// Why a presentation did not answer the request, as against not holding up at all.
///
/// **A separate vocabulary**, because these are about the exchange and not about the credential:
/// a holder who refused an optional attribute has answered the request, and one who left out a
/// required one has not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unanswered {
    /// Something the request required is not in what was shown.
    Missing(Name),
    /// Something was shown that the request did not ask for.
    ///
    /// **Refused rather than kept.** A verifier that quietly accepted more than it asked for would
    /// be one that ends up holding what it never justified needing, which is the whole thing the
    /// catalogue exists to make visible.
    Unasked(String),
    /// What the holder signed as the purpose is not what this verifier asked for.
    PurposeMoved,
}

/// What verifying a presentation against a request concluded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Says {
    /// It holds up and answers the request, and this is what was shown.
    Proved(Box<almena_credential::verify::Shown>),
    /// It holds up and does not answer the request.
    DoesNotAnswer(Unanswered),
    /// It does not hold up, and this is what is wrong with it.
    NotValid(Fault),
    /// Nothing is wrong with it as far as anybody got, and this is what could not be reached.
    ///
    /// **Never shown as invalidity** (`SPECS.md §17.12`). A verifier that conflated the two teaches
    /// its staff to wave people through when the network fails.
    CouldNotVerify(Missing),
}

/// Everything a verifier brings to one check that is not the presentation itself.
///
/// A struct rather than a run of arguments, because four of the five are things the caller went and
/// found out, and getting two of them the wrong way round is a mistake nothing would catch.
#[derive(Debug, Clone)]
pub struct Against<'a> {
    /// The request that asked for it, which names the template by hash.
    pub request: &'a Request,
    /// What the record answered about the issuer.
    pub resolved: &'a Resolved,
    /// What this verifier insists on, which is its own to decide.
    pub policy: &'a Policy<'a>,
    /// What is known about whether the credential has been revoked.
    pub revocation: Revocation,
    /// The epoch this is being verified in.
    pub now: Epoch,
}

/// Verify a presentation against the request that asked for it.
#[must_use]
pub fn verify(presented: &str, against: &Against<'_>) -> Says {
    let Against {
        request,
        resolved,
        policy,
        revocation,
        now,
    } = against;
    let accepts: Vec<&str> = request.accepts.iter().map(Name::as_str).collect();
    let facts = Facts {
        now: *now,
        issuance_key: resolved.issuance_key,
        issuer_closed: resolved.closed,
        methods: policy.methods,
        demands: almena_credential::verify::Demands {
            revocable: policy.revocable,
            closed_issuers: policy.closed_issuers,
        },
        revocation: *revocation,
        nonce: &request.nonce,
        audience: &request.audience,
        // **The shapes this verifier takes, by hash.** A credential issued against another is not
        // one it asked for, whatever it happens to hold.
        accepts: &accepts,
    };

    match check(presented, &facts) {
        Outcome::NotValid(fault) => Says::NotValid(fault),
        Outcome::CouldNotVerify(missing) => Says::CouldNotVerify(missing),
        Outcome::Valid(shown) => match answers(&shown, request) {
            Ok(()) => Says::Proved(shown),
            Err(why) => Says::DoesNotAnswer(why),
        },
    }
}

/// Whether what was shown answers what was asked.
fn answers(shown: &almena_credential::verify::Shown, request: &Request) -> Result<(), Unanswered> {
    for wanted in &request.wants {
        // **Only the required ones.** Refusing an optional attribute and having the flow carry on
        // is what makes selective disclosure worth anything in practice (`SPECS.md §9.2`).
        if wanted.required && !shown.attributes.contains_key(wanted.attribute.as_str()) {
            return Err(Unanswered::Missing(wanted.attribute.clone()));
        }
    }
    for name in shown.attributes.keys() {
        if !request
            .wants
            .iter()
            .any(|wanted| wanted.attribute.as_str() == name)
        {
            return Err(Unanswered::Unasked(name.clone()));
        }
    }
    // What the holder signed is what the verifier declared, or the two are not talking about the
    // same exchange — and it is the signed half that says what was consented to.
    let asked = purposes(request);
    if shown
        .purpose
        .iter()
        .any(|(name, what)| asked.get(name) != Some(what))
    {
        return Err(Unanswered::PurposeMoved);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{Against, Policy, Resolved, Says, Unanswered, verify};
    use crate::request::{Request, Wanted};
    use almena_credential::issue::sign;
    use almena_credential::present::{Asked as Asking, show};
    use almena_credential::verify::{Fault, Missing, Revocation};
    use almena_credential::{About, Method, Proof, Status};
    use almena_format::identifier::Name;
    use almena_store::template::How;
    use almena_suite::p256;
    use almena_time::Epoch;
    use std::collections::BTreeMap;

    fn key(seed: u8) -> p256::SigningKey {
        p256::SigningKey::from_secret([seed; 32]).expect("a key")
    }

    fn named(seed: u8) -> Name {
        Name::of(&[seed; 8])
    }

    const METHODS: &[Method] = &[Method::Almena];

    fn policy<'a>() -> Policy<'a> {
        Policy {
            methods: METHODS,
            revocable: false,
            closed_issuers: false,
        }
    }

    fn request(wants: &[(u8, bool)]) -> Request {
        Request {
            template: named(99),
            accepts: vec![named(88)],
            nonce: "a-nonce".to_owned(),
            audience: "did:almena:dev:zAVerifier".to_owned(),
            wants: wants
                .iter()
                .map(|(seed, required)| Wanted {
                    attribute: named(*seed),
                    how: How::Value,
                    required: *required,
                    from_credential: true,
                    purpose: format!("because of {seed}"),
                })
                .collect(),
        }
    }

    /// A presentation of a credential carrying both attributes, showing those named.
    fn presented(request: &Request, showing: &[u8]) -> String {
        let issued = sign(
            &About {
                issuer: "did:almena:dev:zAnIssuer".to_owned(),
                template: named(88).as_str().to_owned(),
                issued: Epoch::new(100),
                expires: Epoch::new(10_000),
                proof: Proof::Disclosure,
                method: Method::Almena,
                status: Status::NotRevocable,
            },
            "credential-one",
            &BTreeMap::from([
                (named(1).as_str().to_owned(), serde_json::json!("Ada")),
                (named(2).as_str().to_owned(), serde_json::json!(true)),
            ]),
            &key(2).verifying_key(),
            &key(1),
        )
        .expect("issued");

        let names: Vec<String> = showing
            .iter()
            .map(|seed| named(*seed).as_str().to_owned())
            .collect();
        let showing: Vec<&str> = names.iter().map(String::as_str).collect();
        show(
            &issued,
            &showing,
            &Asking {
                nonce: request.nonce.clone(),
                audience: request.audience.clone(),
                at: Epoch::new(200),
                purpose: crate::request::purposes(request),
            },
            &key(2),
        )
        .expect("presented")
        .written
    }

    fn resolved() -> Resolved {
        Resolved {
            issuance_key: Some(key(1).verifying_key()),
            closed: false,
        }
    }

    #[test]
    fn a_presentation_that_answers_the_request_is_what_was_asked_for() {
        let request = request(&[(1, true), (2, false)]);
        let held = presented(&request, &[1, 2]);
        let Says::Proved(shown) = verify(
            &held,
            &Against {
                request: &request,
                resolved: &resolved(),
                policy: &policy(),
                revocation: Revocation::NothingToCheck,
                now: Epoch::new(200),
            },
        ) else {
            panic!("it proves what was asked")
        };
        assert_eq!(shown.attributes.len(), 2);
    }

    #[test]
    fn refusing_an_optional_attribute_still_answers_the_request() {
        // **What makes selective disclosure worth anything in practice** (`SPECS.md §9.2`). If a
        // flow did not survive a refusal, the only real choice would be all or nothing.
        let request = request(&[(1, true), (2, false)]);
        let held = presented(&request, &[1]);
        assert!(matches!(
            verify(
                &held,
                &Against {
                    request: &request,
                    resolved: &resolved(),
                    policy: &policy(),
                    revocation: Revocation::NothingToCheck,
                    now: Epoch::new(200),
                },
            ),
            Says::Proved(_)
        ));
    }

    #[test]
    fn leaving_out_something_required_does_not_answer_it() {
        let request = request(&[(1, true), (2, true)]);
        let held = presented(&request, &[1]);
        assert_eq!(
            verify(
                &held,
                &Against {
                    request: &request,
                    resolved: &resolved(),
                    policy: &policy(),
                    revocation: Revocation::NothingToCheck,
                    now: Epoch::new(200),
                },
            ),
            Says::DoesNotAnswer(Unanswered::Missing(named(2)))
        );
    }

    #[test]
    fn more_than_was_asked_for_is_refused_rather_than_kept() {
        // A verifier that quietly took what it did not ask for would end up holding what it never
        // justified needing, which is the whole thing the catalogue makes visible.
        let asked_for_one = request(&[(1, true)]);
        let showed_two = presented(&request(&[(1, true), (2, false)]), &[1, 2]);
        assert_eq!(
            verify(
                &showed_two,
                &Against {
                    request: &asked_for_one,
                    resolved: &resolved(),
                    policy: &policy(),
                    revocation: Revocation::NothingToCheck,
                    now: Epoch::new(200),
                },
            ),
            Says::DoesNotAnswer(Unanswered::Unasked(named(2).as_str().to_owned()))
        );
    }

    #[test]
    fn what_the_holder_signed_as_the_purpose_is_what_binds() {
        // The purpose lives in the signed half. One that only lived in the request would be one the
        // verifier could restate afterwards.
        let asked = request(&[(1, true)]);
        let mut differently = asked.clone();
        differently.wants[0].purpose = "for something else entirely".to_owned();
        let held = presented(&differently, &[1]);
        assert_eq!(
            verify(
                &held,
                &Against {
                    request: &asked,
                    resolved: &resolved(),
                    policy: &policy(),
                    revocation: Revocation::NothingToCheck,
                    now: Epoch::new(200),
                },
            ),
            Says::DoesNotAnswer(Unanswered::PurposeMoved)
        );
    }

    #[test]
    fn the_three_answers_stay_three() {
        // **`SPECS.md §17.12` as a type.** One of these is the holder's problem at the counter and
        // one is nobody's, and a verifier that showed them the same way teaches its staff to wave
        // people through when the network fails.
        let request = request(&[(1, true)]);
        let held = presented(&request, &[1]);

        let mut unreachable = resolved();
        unreachable.issuance_key = None;
        assert_eq!(
            verify(
                &held,
                &Against {
                    request: &request,
                    resolved: &unreachable,
                    policy: &policy(),
                    revocation: Revocation::NothingToCheck,
                    now: Epoch::new(200),
                },
            ),
            Says::CouldNotVerify(Missing::IssuerUnresolved)
        );
        assert_eq!(
            verify(
                &held,
                &Against {
                    request: &request,
                    resolved: &resolved(),
                    policy: &policy(),
                    revocation: Revocation::Fresh { revoked: true },
                    now: Epoch::new(200),
                },
            ),
            Says::NotValid(Fault::Revoked)
        );
        assert_eq!(
            verify(
                &held,
                &Against {
                    request: &request,
                    resolved: &resolved(),
                    policy: &policy(),
                    revocation: Revocation::Stale,
                    now: Epoch::new(200),
                },
            ),
            Says::CouldNotVerify(Missing::StatusStale)
        );
    }
}
