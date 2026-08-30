//! Signing a credential: the commitments, the payload, and the one signature over them.
//!
//! # What the issuer decides and what it does not
//!
//! It decides what goes in and against which template. It does **not** decide whether the holder
//! keeps it: issuance is not conditional on acceptance, delivery is (`SPECS.md §9.5`). A credential
//! signed and never collected is inert, because presenting one needs the holder's key.
//!
//! # And it binds the credential to a key the holder generated
//!
//! One key per credential, made by the wallet when it accepts (`SPECS.md §9.1`). What arrives here
//! is the public half: the issuer never sees the other one, and a credential without a binding key
//! would be one a thief could present.

use std::collections::BTreeMap;

use almena_suite::p256;

use crate::disclosure::{Disclosure, commitment};
use crate::{ALGORITHM, About, DIGEST_NAME, MEDIA_TYPE, Status, base64url, claim};

/// A credential as it leaves the issuer: the signed part, and everything it commits to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issued {
    /// The JWS: `header.payload.signature`, which is what the issuer signed.
    pub jwt: String,
    /// Every disclosure, in the order they were committed to.
    ///
    /// **They travel with the credential and are the holder's from then on.** What the holder shows
    /// is a subset of these; what it keeps is all of them, because a disclosure it threw away is an
    /// attribute it can never show again.
    pub disclosures: Vec<Disclosure>,
}

impl Issued {
    /// The whole of it, in the one form it is stored and handed over in.
    ///
    /// `<jwt>~<disclosure>~…~` — with the trailing separator, which is what says there is no
    /// key-binding JWT on the end of it yet.
    #[must_use]
    pub fn written(&self) -> String {
        let mut held = self.jwt.clone();
        for one in &self.disclosures {
            held.push('~');
            held.push_str(&one.written());
        }
        held.push('~');
        held
    }
}

/// Why a credential could not be issued.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotIssued {
    /// It carries no attribute at all, which is not a credential but a signature about nothing.
    Empty,
    /// The operating system would not produce randomness, so no salt is unguessable.
    ///
    /// **Refused rather than worked around.** A commitment whose salt can be computed is one that
    /// can be tested against a guess, and half the attributes anybody issues take a handful of
    /// values.
    NoRandomness,
    /// It would already be expired, or expires before it was issued.
    Expired,
    /// Two attributes with one name: one of them could never be shown.
    Twice,
}

/// Sign a credential.
///
/// The attributes are given as names and values; each becomes a disclosure with a salt of its own,
/// and what the issuer signs is the set of commitments. **The identifier goes in as an attribute**
/// rather than in the clear, because an identifier that always travelled would correlate two
/// presentations exactly as an attribute would.
///
/// # Errors
///
/// [`NotIssued`] for a credential that says nothing, one already expired, or one naming an
/// attribute twice.
pub fn sign(
    about: &About,
    identifier: &str,
    attributes: &BTreeMap<String, serde_json::Value>,
    holder: &p256::VerifyingKey,
    key: &p256::SigningKey,
) -> Result<Issued, NotIssued> {
    if attributes.is_empty() {
        return Err(NotIssued::Empty);
    }
    if about.expires.number() <= about.issued.number() {
        return Err(NotIssued::Expired);
    }
    if attributes.contains_key(claim::IDENTIFIER) {
        // The identifier is put in below. An attribute that took its name would silently replace
        // it, and the credential would have two things called `jti` or one of them lost.
        return Err(NotIssued::Twice);
    }

    let mut disclosures: Vec<Disclosure> = vec![
        Disclosure::new(
            claim::IDENTIFIER,
            serde_json::Value::String(identifier.to_owned()),
        )
        .map_err(|_| NotIssued::NoRandomness)?,
    ];
    for (name, value) in attributes {
        disclosures
            .push(Disclosure::new(name, value.clone()).map_err(|_| NotIssued::NoRandomness)?);
    }

    // **Sorted, so that the order of the commitments says nothing about the order of the
    // attributes.** The set is what is signed; leaving it in the order somebody happened to build
    // it in would leak which attribute is which to anybody who has seen another credential of the
    // same shape.
    let mut committed: Vec<String> = disclosures
        .iter()
        .map(|one| commitment(&one.written()))
        .collect();
    committed.sort();

    let payload = serde_json::json!({
        claim::ISSUER: about.issuer,
        claim::TEMPLATE: about.template,
        claim::ISSUED: about.issued.number(),
        claim::EXPIRES: about.expires.number(),
        claim::PROOF: about.proof.name(),
        claim::METHOD: about.method.name(),
        claim::STATUS: written(&about.status),
        claim::CONFIRMATION: { "jwk": jwk(holder) },
        claim::DIGEST: DIGEST_NAME,
        claim::COMMITMENTS: committed,
    });

    Ok(Issued {
        jwt: signed(&payload, key, MEDIA_TYPE),
        disclosures,
    })
}

/// A JWS over that payload: `header.payload.signature`, base64url throughout.
#[must_use]
pub fn signed(payload: &serde_json::Value, key: &p256::SigningKey, kind: &str) -> String {
    let header = serde_json::json!({ "alg": ALGORITHM, "typ": kind });
    let over = format!(
        "{}.{}",
        base64url::encode(header.to_string().as_bytes()),
        base64url::encode(payload.to_string().as_bytes())
    );
    let signature = key.sign(over.as_bytes());
    format!("{over}.{}", base64url::encode(&signature.bytes()))
}

/// A key as a JWK, which is how `cnf` carries one.
fn jwk(key: &p256::VerifyingKey) -> serde_json::Value {
    let (x, y) = key.coordinates();
    serde_json::json!({
        "kty": "EC",
        "crv": "P-256",
        "x": base64url::encode(&x),
        "y": base64url::encode(&y),
    })
}

/// The status claim, in both of its shapes.
fn written(status: &Status) -> serde_json::Value {
    match status {
        Status::Revocable { list, index } => serde_json::json!({
            "revocable": true,
            "list": list,
            "index": index,
        }),
        // **Said, never left out.** A reader that concluded *not revocable* from an absent field
        // would accept a revoked credential presented with the mechanism stripped off.
        Status::NotRevocable => serde_json::json!({ "revocable": false }),
    }
}

#[cfg(test)]
mod tests {
    use super::{NotIssued, sign};
    use crate::disclosure::commitment;
    use crate::{About, Method, Proof, Status, base64url, claim};
    use almena_suite::p256;
    use almena_time::Epoch;
    use std::collections::BTreeMap;

    fn key(seed: u8) -> p256::SigningKey {
        p256::SigningKey::from_secret([seed; 32]).expect("a key")
    }

    fn about(status: Status) -> About {
        About {
            issuer: "did:almena:dev:zAnIssuer".to_owned(),
            template: "zAVersion".to_owned(),
            issued: Epoch::new(100),
            expires: Epoch::new(10_000),
            proof: Proof::Disclosure,
            method: Method::Almena,
            status,
        }
    }

    fn attributes() -> BTreeMap<String, serde_json::Value> {
        BTreeMap::from([
            ("given_name".to_owned(), serde_json::json!("Ada")),
            ("age_over_18".to_owned(), serde_json::json!(true)),
        ])
    }

    /// The payload of an issued credential, read back.
    fn payload(jwt: &str) -> serde_json::Value {
        let middle = jwt.split('.').nth(1).expect("three parts");
        serde_json::from_slice(&base64url::decode(middle).expect("base64url")).expect("json")
    }

    #[test]
    fn every_attribute_is_a_commitment_and_the_identifier_is_one_too() {
        // **The identifier is hideable** (`SPECS.md §9.1`). One that always travelled would
        // correlate two presentations exactly as an attribute would, and hiding the attributes
        // while leaving the name in place would be hiding nothing.
        let issued = sign(
            &about(Status::NotRevocable),
            "credential-one",
            &attributes(),
            &key(2).verifying_key(),
            &key(1),
        )
        .expect("issued");

        let held = payload(&issued.jwt);
        assert_eq!(issued.disclosures.len(), 3, "two attributes and the name");
        assert!(
            held.get(claim::IDENTIFIER).is_none(),
            "and the name is not in the clear"
        );
        assert!(held.get("given_name").is_none(), "nor is any attribute");

        let committed = held[claim::COMMITMENTS]
            .as_array()
            .expect("the commitments")
            .clone();
        for one in &issued.disclosures {
            assert!(
                committed.contains(&serde_json::Value::String(commitment(&one.written()))),
                "{} is committed to",
                one.name
            );
        }
    }

    #[test]
    fn the_commitments_are_sorted_so_their_order_says_nothing() {
        // The set is what is signed. Left in the order somebody built it in, the position of a
        // commitment would tell anybody who has seen another credential of the same shape which
        // attribute is which.
        let issued = sign(
            &about(Status::NotRevocable),
            "credential-one",
            &attributes(),
            &key(2).verifying_key(),
            &key(1),
        )
        .expect("issued");
        let held = payload(&issued.jwt);
        let committed: Vec<&str> = held[claim::COMMITMENTS]
            .as_array()
            .expect("the commitments")
            .iter()
            .filter_map(serde_json::Value::as_str)
            .collect();
        let mut sorted = committed.clone();
        sorted.sort_unstable();
        assert_eq!(committed, sorted);
    }

    #[test]
    fn not_being_revocable_is_something_the_issuer_says() {
        // **Never an absent field** (`SPECS.md §10.1`), or an attacker presents a revoked
        // credential by leaving the mechanism out.
        let plain = sign(
            &about(Status::NotRevocable),
            "one",
            &attributes(),
            &key(2).verifying_key(),
            &key(1),
        )
        .expect("issued");
        assert_eq!(payload(&plain.jwt)[claim::STATUS]["revocable"], false);

        let listed = sign(
            &about(Status::Revocable {
                list: "did:almena:dev:zAList".to_owned(),
                index: 4242,
            }),
            "one",
            &attributes(),
            &key(2).verifying_key(),
            &key(1),
        )
        .expect("issued");
        let status = &payload(&listed.jwt)[claim::STATUS];
        assert_eq!(status["revocable"], true);
        assert_eq!(status["index"], 4242);
    }

    #[test]
    fn the_proof_type_and_the_method_are_outside_the_commitments() {
        // If either could be hidden it would not be a mark: the verifier would not fail closed, it
        // would assume the one it knows.
        let issued = sign(
            &about(Status::NotRevocable),
            "one",
            &attributes(),
            &key(2).verifying_key(),
            &key(1),
        )
        .expect("issued");
        let held = payload(&issued.jwt);
        assert_eq!(held[claim::PROOF], "disclosure");
        assert_eq!(held[claim::METHOD], "did:almena");
    }

    #[test]
    fn a_credential_that_says_nothing_or_has_already_expired_is_not_one() {
        let none = BTreeMap::new();
        assert_eq!(
            sign(
                &about(Status::NotRevocable),
                "one",
                &none,
                &key(2).verifying_key(),
                &key(1)
            ),
            Err(NotIssued::Empty)
        );

        let mut backwards = about(Status::NotRevocable);
        backwards.expires = Epoch::new(50);
        assert_eq!(
            sign(
                &backwards,
                "one",
                &attributes(),
                &key(2).verifying_key(),
                &key(1)
            ),
            Err(NotIssued::Expired)
        );

        let mut clashing = attributes();
        clashing.insert(claim::IDENTIFIER.to_owned(), serde_json::json!("mine"));
        assert_eq!(
            sign(
                &about(Status::NotRevocable),
                "one",
                &clashing,
                &key(2).verifying_key(),
                &key(1)
            ),
            Err(NotIssued::Twice)
        );
    }
}
