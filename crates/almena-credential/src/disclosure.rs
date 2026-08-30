//! Selective disclosure by commitment: what the issuer signs, and what the holder chooses to show.
//!
//! # The shape of it
//!
//! `SPECS.md §9.1`. The issuer replaces every attribute with a **commitment** — the hash of the
//! value together with a salt nobody else knows — and signs the set of commitments. The holder
//! shows the value and its salt for what they want to show, and whoever receives it recomputes the
//! hash and finds it in the signed set. What is not shown is a hash of something with a random
//! salt in front of it, which says nothing about what it stands for.
//!
//! # Why a salt per attribute and not one per credential
//!
//! With one salt, a verifier who has seen the value of an attribute once can test it against every
//! other commitment in the credential — *is the surname Garcia?* costs one hash. A salt per
//! attribute makes each commitment its own guessing problem, and the salt is only revealed with
//! the value it belongs to.
//!
//! # And the salt is 128 bits, which is not a round number chosen for looks
//!
//! It is what stops a commitment over a **low-entropy value** being brute-forced. Half the
//! attributes in the core are things like a country code or a yes-or-no: the value space is
//! tiny, so all the unguessability there is has to come from the salt.

use almena_suite::digest::Digest;

use crate::base64url;

/// How wide a salt is, in bytes.
///
/// **Sixteen**, which is the SD-JWT specification's own recommendation and the width that makes a
/// commitment over a one-bit value as hard to guess as one over a name.
pub const SALT_WIDTH: usize = 16;

/// One attribute, as it travels when the holder chooses to show it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Disclosure {
    /// The salt, which is what makes the commitment unguessable.
    pub salt: [u8; SALT_WIDTH],
    /// The claim's name.
    pub name: String,
    /// Its value, as JSON — a string, a number, a boolean or something nested.
    pub value: serde_json::Value,
}

/// Why a disclosure could not be read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotADisclosure {
    /// The text is not base64url, or not canonical base64url.
    NotBase64Url,
    /// What it decodes to is not JSON.
    NotJson,
    /// It is JSON and not the three-element array a disclosure is.
    Shape,
    /// The salt is not the width every salt is.
    Salt,
}

impl Disclosure {
    /// One attribute, with a salt drawn from the operating system.
    ///
    /// **Generated and never derived.** A salt derived from anything — the value, the credential, a
    /// counter — would be one somebody else could compute, and a commitment whose salt can be
    /// computed is a commitment that can be tested against a guess.
    ///
    /// # Errors
    ///
    /// [`getrandom::Error`] when the operating system will not produce randomness. Refused rather
    /// than worked around: a machine that cannot produce an unguessable salt cannot produce a
    /// commitment worth anything, and issuing anyway would be issuing a credential whose attributes
    /// can be guessed one at a time.
    pub fn new(name: &str, value: serde_json::Value) -> Result<Self, getrandom::Error> {
        let mut salt = [0u8; SALT_WIDTH];
        getrandom::fill(&mut salt)?;
        Ok(Self {
            salt,
            name: name.to_owned(),
            value,
        })
    }

    /// How it travels: base64url over the JSON array `[salt, name, value]`.
    ///
    /// **The text is the disclosure**, not the array it stands for. The commitment is the hash of
    /// these characters, so whoever holds it hands on exactly what it received rather than
    /// re-encoding — two encoders that space a JSON array differently would produce two digests of
    /// one attribute, and only one of them is in what the issuer signed.
    #[must_use]
    pub fn written(&self) -> String {
        let held = serde_json::Value::Array(vec![
            serde_json::Value::String(base64url::encode(&self.salt)),
            serde_json::Value::String(self.name.clone()),
            self.value.clone(),
        ]);
        base64url::encode(held.to_string().as_bytes())
    }

    /// One read back from how it was written.
    ///
    /// # Errors
    ///
    /// [`NotADisclosure`], telling apart text that is not base64url from bytes that are not JSON
    /// and from JSON that is not the shape a disclosure has.
    pub fn read(text: &str) -> Result<Self, NotADisclosure> {
        let bytes = base64url::decode(text).map_err(|_| NotADisclosure::NotBase64Url)?;
        let held: serde_json::Value =
            serde_json::from_slice(&bytes).map_err(|_| NotADisclosure::NotJson)?;
        let serde_json::Value::Array(three) = held else {
            return Err(NotADisclosure::Shape);
        };
        let [
            serde_json::Value::String(salt),
            serde_json::Value::String(name),
            value,
        ] = three.as_slice()
        else {
            return Err(NotADisclosure::Shape);
        };
        let salt = base64url::decode(salt).map_err(|_| NotADisclosure::NotBase64Url)?;
        Ok(Self {
            salt: salt.try_into().map_err(|_| NotADisclosure::Salt)?,
            name: name.clone(),
            value: value.clone(),
        })
    }
}

/// The commitment that stands for a disclosure in what the issuer signed.
///
/// **Taken over the text and not over the value.** What is in the signed set is the digest of the
/// characters that travel, so recomputing it needs no agreement about how JSON is spaced.
#[must_use]
pub fn commitment(written: &str) -> String {
    base64url::encode(Digest::of(written.as_bytes()).bytes())
}

#[cfg(test)]
mod tests {
    use super::{Disclosure, NotADisclosure, commitment};
    use crate::base64url;

    #[test]
    fn a_disclosure_reads_back_as_itself() {
        let one = Disclosure::new("given_name", serde_json::json!("Ada")).expect("randomness");
        let read = Disclosure::read(&one.written()).expect("a disclosure");
        assert_eq!(read, one);
    }

    #[test]
    fn two_disclosures_of_one_value_commit_to_two_different_things() {
        // **What the salt is for.** Without it, a verifier who has seen a value once could test it
        // against every commitment in every credential that carries it.
        let one = Disclosure::new("nationality", serde_json::json!("ES")).expect("randomness");
        let other = Disclosure::new("nationality", serde_json::json!("ES")).expect("randomness");
        assert_ne!(one.salt, other.salt);
        assert_ne!(commitment(&one.written()), commitment(&other.written()));
    }

    #[test]
    fn the_commitment_is_over_the_text_that_travels() {
        // Whoever receives a disclosure hands on exactly what it received: two encoders spacing one
        // JSON array differently would produce two digests of one attribute, and only one of them
        // would be in what the issuer signed.
        let one = Disclosure::new("age_over_18", serde_json::json!(true)).expect("randomness");
        let written = one.written();
        assert_eq!(commitment(&written), commitment(&written.clone()));
        assert_ne!(commitment(&written), commitment(&format!("{written}A")));
    }

    #[test]
    fn something_that_is_not_a_disclosure_is_refused_and_says_which_way() {
        assert_eq!(
            Disclosure::read("not base64url!"),
            Err(NotADisclosure::NotBase64Url)
        );
        assert_eq!(
            Disclosure::read(&base64url::encode(b"not json at all")),
            Err(NotADisclosure::NotJson)
        );
        assert_eq!(
            Disclosure::read(&base64url::encode(br#"["only","two"]"#)),
            Err(NotADisclosure::Shape)
        );
        assert_eq!(
            Disclosure::read(&base64url::encode(br#"["c2hvcnQ","name","value"]"#)),
            Err(NotADisclosure::Salt),
            "a salt narrower than every salt is not one"
        );
    }
}
