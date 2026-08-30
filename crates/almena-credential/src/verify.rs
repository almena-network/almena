//! Verifying a presentation, and the difference between *not valid* and *could not be verified*.
//!
//! # The distinction is a type here, not a message
//!
//! `SPECS.md §17.12` makes it a conformance requirement: a verifier must tell *could not be
//! verified* from *not valid* on screen, because if it conflates them its staff learn to wave
//! people through when the network fails. Making it a message would make it a thing somebody
//! remembers to write; making it the return type makes it a thing they cannot avoid handling.
//!
//! **They are opposite failures.** *Not valid* is about the credential and is the holder's problem;
//! *could not be verified* is about the verifier's own reach and is nobody's fault at the counter.
//! One is a refusal, the other is a *come back in a minute* — and treating a network outage as
//! forgery is how a real document gets rejected.
//!
//! # Nothing is fetched from here
//!
//! Whoever calls this brings the facts: the issuer's key resolved from the record, whether the
//! issuer is closed, what the status list says and how fresh it is. That keeps this a function of
//! its inputs — testable to the end — and it keeps the fetching where the policy lives, because
//! *how fresh is fresh* and *how many nodes to ask* are the verifier's own decisions
//! (`SPECS.md §4.4`, `§10.2`).
//!
//! # And what decides *with what* is never taken from what is being verified
//!
//! The issuer identification method travels in the credential and is read before anything has been
//! checked. So the verifier accepts the methods on **its own list** and refuses everything else
//! (`SPECS.md §9.1`) — otherwise what has not been verified chooses what verifies it, which is
//! algorithm confusion with a new coat on.

use std::collections::BTreeMap;

use almena_suite::digest::Digest;
use almena_suite::p256;
use almena_time::Epoch;

use crate::disclosure::{Disclosure, commitment};
use crate::present::{Parts, parts};
use crate::{ALGORITHM, BINDING_TYPE, MEDIA_TYPE, Method, Proof, base64url, claim};

/// What the record and the network said, brought by whoever is verifying.
#[derive(Debug, Clone)]
pub struct Facts<'a> {
    /// The epoch this is being verified in.
    pub now: Epoch,
    /// The key the record says the issuer emits with.
    ///
    /// **[`None`] is *nobody could be asked*** and never *the issuer does not exist*: the first is
    /// about this verifier's reach and the second is a claim about the record.
    pub issuance_key: Option<p256::VerifyingKey>,
    /// Whether the record says the issuer's organisation is closed.
    pub issuer_closed: bool,
    /// The identification methods this verifier accepts. **Its own list.**
    pub methods: &'a [Method],
    /// What the verifier requires of a credential before it will look at what is in it.
    pub demands: Demands,
    /// What is known about whether this credential has been revoked.
    pub revocation: Revocation,
    /// The nonce this verifier put in its challenge.
    pub nonce: &'a str,
    /// Who the presentation had to be for, which is this verifier.
    pub audience: &'a str,
    /// The credential shapes this verifier takes the data from, by template version hash.
    ///
    /// **Empty is *any*, and it is a policy and not an oversight.** A verifier that does not
    /// restrict which credential a claim comes from is one taking any issuer's word for it, which
    /// is its own decision to make (`SPECS.md §12.2`) — and the holder sees which credential each
    /// datum came out of either way (`SPECS.md §9.2`).
    ///
    /// **By hash** (`SPECS.md §9.4`): a shape named by anything a later act could change would be
    /// one whose meaning moves between the asking and the answering.
    pub accepts: &'a [&'a str],
}

/// What a verifier insists on, before anything about the credential is read.
///
/// **Its own policy and never the credential's** (`SPECS.md §10.1`, `§12.2`): a verifier may demand
/// that a credential be revocable at all, and decides for itself whether it takes credentials from
/// issuers already closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Demands {
    /// Whether a credential that says it cannot be revoked is refused.
    pub revocable: bool,
    /// Whether a credential from an organisation already closed is still taken.
    pub closed_issuers: bool,
}

/// What is known about a credential's revocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Revocation {
    /// The credential says it cannot be revoked, so there was nothing to look up.
    NothingToCheck,
    /// A list was obtained and it matched the freshest version hash the verifier could see.
    Fresh {
        /// Whether the bit is set.
        revoked: bool,
    },
    /// A list was obtained and it is **older** than the freshest version hash in the record.
    ///
    /// **Never used** (`SPECS.md §10.2`, rule 1). A verifier that has the hash knows when the bytes
    /// it was handed are stale, so accepting a revoked credential *believing the list is current*
    /// cannot happen — what is left is availability, and that is said as availability.
    Stale,
    /// No list could be obtained at all.
    Unavailable,
}

/// Why a credential does not hold up. **About the credential.**
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fault {
    /// It is not a presentation, or not one this build can read.
    Malformed,
    /// The identification method is not one this verifier accepts.
    MethodNotAccepted,
    /// The proof type is not one this build knows — refused, never read as the nearest.
    ProofUnknown,
    /// The issuer's signature is not the issuer's.
    BadSignature,
    /// Its validity has run out.
    Expired,
    /// It says it was issued later than now.
    NotYetIssued,
    /// The organisation that issued it is closed, and this verifier does not take those.
    IssuerClosed,
    /// It was issued against a different template version than the request authorises.
    WrongTemplate,
    /// It says it cannot be revoked, and this verifier requires that credentials can be.
    NotRevocable,
    /// The issuer has revoked it.
    Revoked,
    /// A disclosure shown is not one the issuer committed to.
    NotCommitted,
    /// It arrived with no key-binding signature, so nothing says the holder is the holder.
    NotBound,
    /// The binding answers a different challenge, or is addressed to somebody else.
    WrongChallenge,
    /// The binding does not cover this set of disclosures.
    BindingDoesNotCover,
    /// The binding signature is not the bound key's.
    BindingFailed,
}

/// Why verification could not be finished. **About the verifier's reach, never the credential.**
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Missing {
    /// The issuer could not be resolved, so there is no key to check the signature against.
    IssuerUnresolved,
    /// The only status list obtainable is older than the freshest version in the record.
    StatusStale,
    /// No status list could be obtained at all.
    StatusUnavailable,
}

/// What a presentation showed, once it held up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shown {
    /// Who issued it.
    pub issuer: String,
    /// Which template version it was issued against.
    pub template: String,
    /// The attributes the holder chose to show.
    pub attributes: BTreeMap<String, serde_json::Value>,
    /// What each was said to be for, as the verifier declared it and the holder signed it.
    pub purpose: BTreeMap<String, String>,
    /// The epoch it stops being valid in.
    pub expires: Epoch,
}

/// What verifying concluded.
///
/// **Three answers and not two.** The third is the whole point of `SPECS.md §17.12`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// It holds up, and this is what it showed.
    Valid(Box<Shown>),
    /// It does not hold up, and this is what is wrong with it.
    NotValid(Fault),
    /// Nothing is wrong with it as far as anybody got, and this is what could not be reached.
    CouldNotVerify(Missing),
}

/// Verify a presentation against the facts brought with it.
#[must_use]
pub fn check(written: &str, facts: &Facts<'_>) -> Outcome {
    let Some(taken) = parts(written) else {
        return Outcome::NotValid(Fault::Malformed);
    };
    let Some(payload) = payload_of(taken.jwt, MEDIA_TYPE) else {
        return Outcome::NotValid(Fault::Malformed);
    };

    // **Before anything is checked, what to check with.** From the verifier's own list, never from
    // what the credential proposes.
    match Method::of(payload[claim::METHOD].as_str().unwrap_or_default()) {
        Some(method) if facts.methods.contains(&method) => {}
        _ => return Outcome::NotValid(Fault::MethodNotAccepted),
    }
    if Proof::of(payload[claim::PROOF].as_str().unwrap_or_default()).is_none() {
        return Outcome::NotValid(Fault::ProofUnknown);
    }

    // The key comes from the record. Not having reached it is not the credential's fault, and
    // saying so as though it were is what §17.12 exists to stop.
    let Some(issuance) = facts.issuance_key else {
        return Outcome::CouldNotVerify(Missing::IssuerUnresolved);
    };
    if !signature_holds(taken.jwt, &issuance) {
        return Outcome::NotValid(Fault::BadSignature);
    }

    if let Some(fault) = what_it_says(&payload, facts) {
        return Outcome::NotValid(fault);
    }
    match facts.revocation {
        Revocation::Fresh { revoked: true } => return Outcome::NotValid(Fault::Revoked),
        Revocation::Stale => return Outcome::CouldNotVerify(Missing::StatusStale),
        Revocation::Unavailable => return Outcome::CouldNotVerify(Missing::StatusUnavailable),
        Revocation::Fresh { revoked: false } | Revocation::NothingToCheck => {}
    }

    let attributes = match disclosed(&taken, &payload) {
        Ok(held) => held,
        Err(fault) => return Outcome::NotValid(fault),
    };
    let purpose = match bound(&taken, &payload, facts) {
        Ok(held) => held,
        Err(fault) => return Outcome::NotValid(fault),
    };

    Outcome::Valid(Box::new(Shown {
        issuer: payload[claim::ISSUER]
            .as_str()
            .unwrap_or_default()
            .to_owned(),
        template: payload[claim::TEMPLATE]
            .as_str()
            .unwrap_or_default()
            .to_owned(),
        attributes,
        purpose,
        expires: Epoch::new(payload[claim::EXPIRES].as_u64().unwrap_or_default()),
    }))
}

/// What the credential says about itself, held against the moment and the verifier's demands.
fn what_it_says(payload: &serde_json::Value, facts: &Facts<'_>) -> Option<Fault> {
    let (Some(issued), Some(expires)) = (
        payload[claim::ISSUED].as_u64(),
        payload[claim::EXPIRES].as_u64(),
    ) else {
        return Some(Fault::Malformed);
    };
    if facts.now.number() >= expires {
        return Some(Fault::Expired);
    }
    if facts.now.number() < issued {
        return Some(Fault::NotYetIssued);
    }
    if facts.issuer_closed && !facts.demands.closed_issuers {
        return Some(Fault::IssuerClosed);
    }
    // **One of the shapes this verifier takes, by hash.** A credential issued against another is
    // not one it asked for, whatever it happens to hold.
    let against = payload[claim::TEMPLATE].as_str().unwrap_or_default();
    if !facts.accepts.is_empty() && !facts.accepts.contains(&against) {
        return Some(Fault::WrongTemplate);
    }

    let status = &payload[claim::STATUS];
    match status["revocable"].as_bool() {
        // **Absence is not *not revocable***: an attacker would present a revoked credential by
        // stripping the mechanism off.
        None => Some(Fault::Malformed),
        Some(false) if facts.demands.revocable => Some(Fault::NotRevocable),
        Some(_) => None,
    }
}

/// The attributes shown, each held against a commitment the issuer signed.
fn disclosed(
    taken: &Parts<'_>,
    payload: &serde_json::Value,
) -> Result<BTreeMap<String, serde_json::Value>, Fault> {
    let Some(committed) = payload[claim::COMMITMENTS].as_array() else {
        return Err(Fault::Malformed);
    };
    // The hash the commitments were taken with is named inside the credential rather than assumed,
    // and one this build does not know stops the reader.
    if payload[claim::DIGEST].as_str() != Some(crate::DIGEST_NAME) {
        return Err(Fault::Malformed);
    }

    let mut held = BTreeMap::new();
    for written in &taken.disclosures {
        let one = Disclosure::read(written).map_err(|_| Fault::Malformed)?;
        // **In the signed set, or it is not this issuer's claim.** A disclosure that is merely
        // well-formed says only that somebody could write JSON.
        if !committed.contains(&serde_json::Value::String(commitment(written))) {
            return Err(Fault::NotCommitted);
        }
        if held.insert(one.name, one.value).is_some() {
            // One attribute shown twice would let the second reading quietly replace the first.
            return Err(Fault::Malformed);
        }
    }
    Ok(held)
}

/// The key-binding signature, and the purposes it covers.
fn bound(
    taken: &Parts<'_>,
    payload: &serde_json::Value,
    facts: &Facts<'_>,
) -> Result<BTreeMap<String, String>, Fault> {
    let binding = taken.binding.ok_or(Fault::NotBound)?;
    let held = payload_of(binding, BINDING_TYPE).ok_or(Fault::Malformed)?;

    // The challenge and the audience together: the first stops a presentation being replayed, the
    // second stops one being relayed to a verifier it was not made for.
    if held["nonce"].as_str() != Some(facts.nonce) || held["aud"].as_str() != Some(facts.audience) {
        return Err(Fault::WrongChallenge);
    }

    // Everything in front of the binding, including the trailing separator — which is what ties the
    // signature to this exact set of disclosures rather than to the credential in general.
    let so_far = binding_covers(taken);
    if held["sd_hash"].as_str() != Some(&base64url::encode(Digest::of(so_far.as_bytes()).bytes())) {
        return Err(Fault::BindingDoesNotCover);
    }

    let key = holder_key(payload).ok_or(Fault::Malformed)?;
    if !signature_holds(binding, &key) {
        return Err(Fault::BindingFailed);
    }

    Ok(held["purpose"]
        .as_object()
        .map(|held| {
            held.iter()
                .filter_map(|(name, what)| {
                    what.as_str().map(|what| (name.clone(), what.to_owned()))
                })
                .collect()
        })
        .unwrap_or_default())
}

/// Everything a key-binding signature is taken over.
fn binding_covers(taken: &Parts<'_>) -> String {
    let mut held = taken.jwt.to_owned();
    for one in &taken.disclosures {
        held.push('~');
        held.push_str(one);
    }
    held.push('~');
    held
}

/// The holder's key, out of `cnf`.
fn holder_key(payload: &serde_json::Value) -> Option<p256::VerifyingKey> {
    let jwk = &payload[claim::CONFIRMATION]["jwk"];
    if jwk["kty"].as_str() != Some("EC") || jwk["crv"].as_str() != Some("P-256") {
        return None;
    }
    let x = base64url::decode(jwk["x"].as_str()?).ok()?;
    let y = base64url::decode(jwk["y"].as_str()?).ok()?;
    if x.len() != 32 || y.len() != 32 {
        return None;
    }
    // Compressed, because that is the one spelling of a key this platform reads: the sign of `y`
    // and the whole of `x`.
    let mut compressed = [0u8; p256::PUBLIC_KEY_WIDTH];
    compressed[0] = if y[31] % 2 == 0 { 0x02 } else { 0x03 };
    compressed[1..].copy_from_slice(&x);
    p256::VerifyingKey::from_bytes(compressed).ok()
}

/// The payload of a JWS whose header says it is that kind, or nothing.
///
/// **The header is checked and not skipped past.** A credential whose header names another
/// algorithm is one somebody is hoping will be verified with the one it names.
fn payload_of(jwt: &str, kind: &str) -> Option<serde_json::Value> {
    let mut pieces = jwt.split('.');
    let (header, payload, signature) = (pieces.next()?, pieces.next()?, pieces.next()?);
    if pieces.next().is_some() || signature.is_empty() {
        return None;
    }
    let header: serde_json::Value =
        serde_json::from_slice(&base64url::decode(header).ok()?).ok()?;
    if header["alg"].as_str() != Some(ALGORITHM) || header["typ"].as_str() != Some(kind) {
        return None;
    }
    serde_json::from_slice(&base64url::decode(payload).ok()?).ok()
}

/// Whether a JWS's signature is that key's, over the two parts in front of it.
fn signature_holds(jwt: &str, key: &p256::VerifyingKey) -> bool {
    let Some((over, signature)) = jwt.rsplit_once('.') else {
        return false;
    };
    let Ok(bytes) = base64url::decode(signature) else {
        return false;
    };
    let Ok(bytes) = <[u8; p256::SIGNATURE_WIDTH]>::try_from(bytes.as_slice()) else {
        return false;
    };
    p256::Signature::from_bytes(bytes)
        .is_ok_and(|signature| key.verify(over.as_bytes(), &signature).is_ok())
}

#[cfg(test)]
mod tests {
    use super::{Demands, Facts, Fault, Missing, Outcome, Revocation, check};
    use crate::disclosure::Disclosure;
    use crate::issue::{Issued, sign};
    use crate::present::{Asked, show};
    use crate::{About, Method, Proof, Status};
    use almena_suite::p256;
    use almena_time::Epoch;
    use std::collections::BTreeMap;

    fn key(seed: u8) -> p256::SigningKey {
        p256::SigningKey::from_secret([seed; 32]).expect("a key")
    }

    /// The issuer's key is 1, the holder's binding key is 2.
    fn issued(status: Status) -> Issued {
        sign(
            &About {
                issuer: "did:almena:dev:zAnIssuer".to_owned(),
                template: "zAVersion".to_owned(),
                issued: Epoch::new(100),
                expires: Epoch::new(10_000),
                proof: Proof::Disclosure,
                method: Method::Almena,
                status,
            },
            "credential-one",
            &BTreeMap::from([
                ("given_name".to_owned(), serde_json::json!("Ada")),
                ("age_over_18".to_owned(), serde_json::json!(true)),
            ]),
            &key(2).verifying_key(),
            &key(1),
        )
        .expect("issued")
    }

    fn asked() -> Asked {
        Asked {
            nonce: "a-nonce".to_owned(),
            audience: "did:almena:dev:zAVerifier".to_owned(),
            at: Epoch::new(200),
            purpose: BTreeMap::from([(
                "age_over_18".to_owned(),
                "selling something age-restricted".to_owned(),
            )]),
        }
    }

    const METHODS: &[Method] = &[Method::Almena];
    const ACCEPTS: &[&str] = &["zAVersion"];

    fn facts<'a>() -> Facts<'a> {
        Facts {
            now: Epoch::new(200),
            issuance_key: Some(key(1).verifying_key()),
            issuer_closed: false,
            methods: METHODS,
            demands: Demands {
                revocable: false,
                closed_issuers: false,
            },
            revocation: Revocation::NothingToCheck,
            nonce: "a-nonce",
            audience: "did:almena:dev:zAVerifier",
            accepts: ACCEPTS,
        }
    }

    /// One presentation of one credential, showing those attributes.
    fn presented(held: &Issued, showing: &[&str]) -> String {
        show(held, showing, &asked(), &key(2))
            .expect("presented")
            .written
    }

    #[test]
    fn a_presentation_that_holds_up_shows_what_was_shown_and_no_more() {
        let held = issued(Status::NotRevocable);
        let Outcome::Valid(shown) = check(&presented(&held, &["age_over_18"]), &facts()) else {
            panic!("it holds up")
        };
        assert_eq!(shown.attributes.len(), 1);
        assert_eq!(shown.attributes["age_over_18"], serde_json::json!(true));
        assert_eq!(shown.template, "zAVersion");
        assert_eq!(
            shown.purpose["age_over_18"], "selling something age-restricted",
            "and what it was said to be for was signed by the holder"
        );
    }

    #[test]
    fn a_node_that_could_not_be_reached_is_never_reported_as_a_bad_credential() {
        // **The whole of `SPECS.md §17.12`.** One is about the credential and is the holder's
        // problem; the other is about this verifier's reach and is nobody's fault at the counter.
        let held = issued(Status::Revocable {
            list: "did:almena:dev:zAList".to_owned(),
            index: 7,
        });
        let written = presented(&held, &["age_over_18"]);

        let mut unresolved = facts();
        unresolved.issuance_key = None;
        assert_eq!(
            check(&written, &unresolved),
            Outcome::CouldNotVerify(Missing::IssuerUnresolved)
        );

        let mut stale = facts();
        stale.revocation = Revocation::Stale;
        assert_eq!(
            check(&written, &stale),
            Outcome::CouldNotVerify(Missing::StatusStale),
            "an old list is never used, and never reported as invalidity"
        );

        let mut gone = facts();
        gone.revocation = Revocation::Unavailable;
        assert_eq!(
            check(&written, &gone),
            Outcome::CouldNotVerify(Missing::StatusUnavailable)
        );

        let mut revoked = facts();
        revoked.revocation = Revocation::Fresh { revoked: true };
        assert_eq!(
            check(&written, &revoked),
            Outcome::NotValid(Fault::Revoked),
            "and a revocation actually seen is the other answer entirely"
        );
    }

    #[test]
    fn what_verifies_is_never_chosen_by_what_is_being_verified() {
        // The method is read before anything has been checked. A verifier that took the credential's
        // word for how to identify its issuer would be letting the unverified pick the verifier.
        let held = issued(Status::NotRevocable);
        let written = presented(&held, &["age_over_18"]);
        let mut nothing_accepted = facts();
        nothing_accepted.methods = &[];
        assert_eq!(
            check(&written, &nothing_accepted),
            Outcome::NotValid(Fault::MethodNotAccepted)
        );
    }

    #[test]
    fn a_disclosure_the_issuer_never_committed_to_is_not_this_issuers_claim() {
        // Being well-formed says only that somebody could write JSON.
        let held = issued(Status::NotRevocable);
        let written = presented(&held, &["age_over_18"]);
        let invented =
            Disclosure::new("taxID", serde_json::json!("Z-0000000")).expect("randomness");
        // Slipped in in front of the key-binding JWT, where a disclosure goes.
        let (front, binding) = written.rsplit_once('~').expect("a binding");
        let forged = format!("{front}~{}~{binding}", invented.written());
        assert_eq!(
            check(&forged, &facts()),
            Outcome::NotValid(Fault::NotCommitted)
        );
    }

    #[test]
    fn a_binding_lifted_from_one_presentation_does_not_fit_another() {
        // The signature covers everything in front of it, so the same credential cannot be made to
        // show more than the holder agreed to show.
        let held = issued(Status::NotRevocable);
        let little = presented(&held, &["age_over_18"]);
        let more = presented(&held, &["age_over_18", "given_name"]);

        let binding = little.rsplit_once('~').expect("a binding").1;
        let front = more.rsplit_once('~').expect("a binding").0;
        assert_eq!(
            check(&format!("{front}~{binding}"), &facts()),
            Outcome::NotValid(Fault::BindingDoesNotCover)
        );
    }

    #[test]
    fn a_presentation_for_somebody_else_is_not_this_verifiers_to_accept() {
        // Without the audience, a verifier could relay what it received to a second one and pass
        // as the holder; without the nonce, yesterday's presentation would work today.
        let held = issued(Status::NotRevocable);
        let written = presented(&held, &["age_over_18"]);

        let mut somebody_else = facts();
        somebody_else.audience = "did:almena:dev:zAnotherVerifier";
        assert_eq!(
            check(&written, &somebody_else),
            Outcome::NotValid(Fault::WrongChallenge)
        );

        let mut another_challenge = facts();
        another_challenge.nonce = "a-different-nonce";
        assert_eq!(
            check(&written, &another_challenge),
            Outcome::NotValid(Fault::WrongChallenge)
        );
    }

    #[test]
    fn a_credential_handed_over_without_a_signature_proves_nothing() {
        // **What makes a stolen credential worth nothing** (`SPECS.md §9.1`), and what `§9.5`
        // depends on when it says a credential signed and never collected is inert.
        let held = issued(Status::NotRevocable);
        assert_eq!(
            check(&held.written(), &facts()),
            Outcome::NotValid(Fault::NotBound)
        );
    }

    #[test]
    fn the_verifiers_own_demands_are_the_verifiers() {
        let plain = issued(Status::NotRevocable);
        let written = presented(&plain, &["age_over_18"]);

        let mut wants_revocable = facts();
        wants_revocable.demands.revocable = true;
        assert_eq!(
            check(&written, &wants_revocable),
            Outcome::NotValid(Fault::NotRevocable),
            "a verifier may demand that a credential can be revoked"
        );

        let mut closed = facts();
        closed.issuer_closed = true;
        assert_eq!(
            check(&written, &closed),
            Outcome::NotValid(Fault::IssuerClosed)
        );
        closed.demands.closed_issuers = true;
        assert!(
            matches!(check(&written, &closed), Outcome::Valid(_)),
            "and another verifier may decide it takes them"
        );
    }

    #[test]
    fn a_credential_against_another_template_is_not_the_shape_that_was_asked_for() {
        let held = issued(Status::NotRevocable);
        let written = presented(&held, &["age_over_18"]);
        const ELSEWHERE: &[&str] = &["zSomeOtherVersion"];
        let mut wanted = facts();
        wanted.accepts = ELSEWHERE;
        assert_eq!(
            check(&written, &wanted),
            Outcome::NotValid(Fault::WrongTemplate)
        );
    }

    #[test]
    fn expiry_is_read_and_it_is_the_credentials_own_claim() {
        let held = issued(Status::NotRevocable);
        let written = presented(&held, &["age_over_18"]);

        let mut later = facts();
        later.now = Epoch::new(10_000);
        assert_eq!(check(&written, &later), Outcome::NotValid(Fault::Expired));

        let mut earlier = facts();
        earlier.now = Epoch::new(50);
        assert_eq!(
            check(&written, &earlier),
            Outcome::NotValid(Fault::NotYetIssued)
        );
    }

    #[test]
    fn somebody_elses_signature_is_not_the_issuers() {
        let held = issued(Status::NotRevocable);
        let written = presented(&held, &["age_over_18"]);
        let mut wrong = facts();
        wrong.issuance_key = Some(key(9).verifying_key());
        assert_eq!(
            check(&written, &wrong),
            Outcome::NotValid(Fault::BadSignature)
        );
    }

    #[test]
    fn something_that_is_not_a_presentation_is_told_apart_from_one_that_does_not_hold_up() {
        assert_eq!(check("", &facts()), Outcome::NotValid(Fault::Malformed));
        assert_eq!(
            check("not.a.jwt~~", &facts()),
            Outcome::NotValid(Fault::Malformed)
        );
    }
}
