//! Showing a credential: which disclosures go, and the signature that makes it this holder's.
//!
//! # What a presentation is
//!
//! The signed credential, the disclosures the holder chose, and a **key-binding JWT** on the end
//! signed with the key the credential is bound to. That last part is what makes a stolen credential
//! worth nothing (`SPECS.md §9.1`), and it is what `SPECS.md §9.5` depends on when it says a
//! credential signed and never collected is inert.
//!
//! # The challenge is the verifier's and the binding covers the whole thing
//!
//! The key-binding JWT carries the verifier's nonce, who it is for, and a hash of everything in
//! front of it. Without the hash, somebody who saw one presentation could take its binding
//! signature and put it on a **different set of disclosures** — the same credential showing more
//! than the holder agreed to show. Without the audience, a verifier could relay a presentation it
//! received to a second verifier and pass as the holder.
//!
//! # And the declared purpose is signed inside it
//!
//! `SPECS.md §9.2` asks for a purpose **per attribute**, signed within the presentation. Signed by
//! the holder rather than only by the verifier: what is being agreed to is *this data, for this
//! stated reason*, and a purpose that lived only in the request would be one the verifier could
//! restate afterwards.

use std::collections::BTreeMap;

use almena_suite::digest::Digest;
use almena_suite::p256;
use almena_time::Epoch;

use crate::disclosure::Disclosure;
use crate::issue::{Issued, signed};
use crate::{BINDING_TYPE, base64url};

/// What the verifier asked, as the holder answers it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asked {
    /// The nonce the verifier put in its challenge, which is what stops a replay.
    pub nonce: String,
    /// Who the presentation is for, so it cannot be relayed to somebody else.
    pub audience: String,
    /// The epoch it is being made in.
    pub at: Epoch,
    /// What each attribute is being asked for, as the verifier declared it.
    ///
    /// **Signed here rather than taken on the verifier's word afterwards** (`SPECS.md §9.2`). It is
    /// what makes *this data, for this reason* the thing that was agreed to.
    pub purpose: BTreeMap<String, String>,
}

/// A presentation, ready to hand over.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Presented {
    /// The whole of it: `<jwt>~<disclosure>~…~<key-binding jwt>`.
    pub written: String,
}

/// Why a presentation could not be made.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotPresented {
    /// One of the attributes asked for is not in this credential.
    NotHeld,
}

/// Show a credential, revealing exactly the attributes named and no others.
///
/// **Named rather than counted**: the holder says which attributes go, and anything not named stays
/// a hash of something with a random salt in front of it.
///
/// # Errors
///
/// [`NotPresented::NotHeld`] when an attribute named is not one this credential carries — refused
/// rather than quietly left out, because a presentation missing something the holder meant to send
/// is one the verifier reads as a holder who refused it.
pub fn show(
    held: &Issued,
    showing: &[&str],
    asked: &Asked,
    binding: &p256::SigningKey,
) -> Result<Presented, NotPresented> {
    let mut chosen: Vec<&Disclosure> = Vec::with_capacity(showing.len());
    for name in showing {
        chosen.push(
            held.disclosures
                .iter()
                .find(|one| one.name == *name)
                .ok_or(NotPresented::NotHeld)?,
        );
    }

    let mut so_far = held.jwt.clone();
    for one in chosen {
        so_far.push('~');
        so_far.push_str(&one.written());
    }
    so_far.push('~');

    // **Over everything in front of it, including the trailing separator**, which is what ties the
    // signature to this exact set of disclosures rather than to the credential in general.
    let payload = serde_json::json!({
        "nonce": asked.nonce,
        "aud": asked.audience,
        "iat": asked.at.number(),
        "sd_hash": base64url::encode(Digest::of(so_far.as_bytes()).bytes()),
        "purpose": asked.purpose,
    });
    Ok(Presented {
        written: format!("{so_far}{}", signed(&payload, binding, BINDING_TYPE)),
    })
}

/// What a presentation is made of, taken apart without anything being checked.
///
/// **Reading is not verifying.** This says what the pieces are; whether they hold up is
/// [`crate::verify`], and keeping the two apart is what stops a reader concluding anything from
/// having managed to parse something.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parts<'a> {
    /// The issuer's signed part.
    pub jwt: &'a str,
    /// The disclosures the holder chose to show, as they were written.
    pub disclosures: Vec<&'a str>,
    /// The key-binding JWT, where there is one.
    ///
    /// **[`None`] is a credential that has not been presented**, which is a different thing from
    /// one presented without binding: the trailing separator says there is nothing after it.
    pub binding: Option<&'a str>,
}

/// Take a presentation apart.
///
/// **Nothing rather than a reason**: without the separators there is no presentation here at all,
/// and there is only one way for that to be true.
#[must_use]
pub fn parts(written: &str) -> Option<Parts<'_>> {
    let mut pieces = written.split('~');
    let jwt = pieces.next()?;
    let rest: Vec<&str> = pieces.collect();
    // The trailing separator is required: without it, the last disclosure and a key-binding JWT
    // look the same, and *what did the holder show* would depend on guessing.
    let (last, disclosures) = rest.split_last()?;
    Some(Parts {
        jwt,
        disclosures: disclosures.to_vec(),
        binding: if last.is_empty() { None } else { Some(last) },
    })
}

#[cfg(test)]
mod tests {
    use super::{Asked, NotPresented, parts, show};
    use crate::issue::sign;
    use crate::{About, Method, Proof, Status};
    use almena_suite::p256;
    use almena_time::Epoch;
    use std::collections::BTreeMap;

    fn key(seed: u8) -> p256::SigningKey {
        p256::SigningKey::from_secret([seed; 32]).expect("a key")
    }

    fn issued() -> crate::issue::Issued {
        sign(
            &About {
                issuer: "did:almena:dev:zAnIssuer".to_owned(),
                template: "zAVersion".to_owned(),
                issued: Epoch::new(100),
                expires: Epoch::new(10_000),
                proof: Proof::Disclosure,
                method: Method::Almena,
                status: Status::NotRevocable,
            },
            "one",
            &BTreeMap::from([
                ("given_name".to_owned(), serde_json::json!("Ada")),
                ("family_name".to_owned(), serde_json::json!("Lovelace")),
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
                "to sell you something age-restricted".to_owned(),
            )]),
        }
    }

    #[test]
    fn only_what_was_named_goes_and_the_rest_stays_a_hash() {
        // Selective disclosure is worth nothing if showing one attribute shows the others.
        let held = issued();
        let shown = show(&held, &["age_over_18"], &asked(), &key(2)).expect("presented");
        let taken = parts(&shown.written).expect("a presentation");
        assert_eq!(taken.disclosures.len(), 1);
        assert!(taken.binding.is_some());
        assert!(!shown.written.contains("Lovelace"), "as text, anywhere");
        let surname = held
            .disclosures
            .iter()
            .find(|one| one.name == "family_name")
            .expect("it is in the credential");
        assert!(
            !shown.written.contains(&surname.written()),
            "and the disclosure it is inside did not travel"
        );
    }

    #[test]
    fn a_credential_nobody_has_presented_has_no_binding_on_the_end() {
        // Two different things, and the trailing separator is what tells them apart: a stored
        // credential, and one presented without a signature.
        let written = issued().written();
        let taken = parts(&written).expect("a credential");
        assert_eq!(taken.binding, None);
        assert_eq!(
            taken.disclosures.len(),
            4,
            "three attributes and the credential's own name, which is a disclosure like any other"
        );
    }

    #[test]
    fn asking_for_something_the_credential_does_not_carry_is_refused() {
        // Refused rather than quietly left out: a presentation missing what the holder meant to
        // send reads to the verifier as a holder who refused it.
        assert_eq!(
            show(&issued(), &["taxID"], &asked(), &key(2)),
            Err(NotPresented::NotHeld)
        );
    }

    #[test]
    fn two_presentations_of_one_credential_bind_to_different_sets() {
        // The binding covers everything in front of it, so a signature seen on one presentation
        // cannot be lifted onto a different set of disclosures.
        let held = issued();
        let one = show(&held, &["age_over_18"], &asked(), &key(2)).expect("presented");
        let more =
            show(&held, &["age_over_18", "given_name"], &asked(), &key(2)).expect("presented");
        let (one, more) = (
            parts(&one.written).expect("one"),
            parts(&more.written).expect("more"),
        );
        assert_ne!(one.binding, more.binding);
    }
}
