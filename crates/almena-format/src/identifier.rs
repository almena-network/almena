//! What an object is called: the hash of the operation that created it.
//!
//! One rule covers everything in the log. **Every object is named by the hash of its creation**;
//! the ones that also have to be resolvable — holder, entity, issuer, verifier, node — wear
//! `did:almena` in front, and the rest are the hash on its own: templates, attributes, sources,
//! status lists, governance proposals and contradictions.
//!
//! The hash goes in **multibase over multihash**, base58btc, the same as `did:key` and `did:peer`.
//! Carrying the hash function *inside* the name is what allows changing it one day without
//! changing the syntax or the method — one more version rather than a migration.
//!
//! # Rotating does not rename
//!
//! The name comes from the creation. Rotating adds an operation to the chain and leaves the name
//! alone, which is what recovery promises: *the root DID stays the same, only the key that
//! controls it changes.* A name derived from the current key would break every credential,
//! certification and issuer link that pointed at the old one, every time somebody rotated.

use almena_suite::digest::Digest;

/// The multicodec code for SHA-256, which is what a multihash puts in front of the digest.
const SHA2_256: u8 = 0x12;

/// The multibase prefix for base58btc.
const BASE58BTC: char = 'z';

/// The alphabet base58btc counts in — no zero, no capital O, no capital I, no lowercase l.
const ALPHABET: &[u8; 58] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

/// The name of an object: the hash of its creation, in multibase over multihash.
///
/// Self-certifying, unassigned, and impossible to collide with. Holding the creation is holding
/// the proof that this is its name.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Name(String);

/// Which network an identifier belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Network {
    /// The real one. Its identifiers carry no mark.
    Production,
    /// Development. Its identifiers say so.
    Development,
}

/// A resolvable identifier: a name, and the network it means something on.
///
/// It orders, and the order means **nothing about the identifiers**. It is there so that anything
/// written down about several of them comes out in one arrangement whoever writes it — canonical
/// bytes are a rule about what may be signed, and a list that came out differently depending on
/// how it was gathered could not be. There is no ranking here and there is not meant to be.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Did {
    network: Network,
    name: Name,
}

/// Why a string was not a `did:almena`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotADid {
    /// Not this method, or not a DID at all.
    Method,
    /// The right method, but the name after it is not one this method can produce.
    Name,
}

impl Name {
    /// The name the canonical bytes of a creation operation give an object.
    ///
    /// Which bytes those are is [`crate::operation`]'s business, and the part that has to be
    /// written down or two honest implementations disagree: **without the `objeto` field and
    /// without `firmas`**.
    #[must_use]
    pub fn of(creation: &[u8]) -> Self {
        let digest = Digest::of(creation);
        let mut multihash = Vec::with_capacity(2 + almena_suite::digest::WIDTH);
        multihash.push(SHA2_256);
        multihash.push(almena_suite::digest::WIDTH as u8);
        multihash.extend_from_slice(digest.bytes());

        let mut name = String::from(BASE58BTC);
        name.push_str(&base58(&multihash));
        Self(name)
    }

    /// The name as it is written.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// A name read back from how it was written.
    ///
    /// # Errors
    ///
    /// [`NotADid::Name`] when the string is not a base58btc multibase, or the multihash inside is
    /// not a SHA-256 of the right length.
    pub fn parse(text: &str) -> Result<Self, NotADid> {
        let body = text.strip_prefix(BASE58BTC).ok_or(NotADid::Name)?;
        let bytes = unbase58(body).ok_or(NotADid::Name)?;
        let [SHA2_256, length, ..] = bytes[..] else {
            return Err(NotADid::Name);
        };
        if usize::from(length) != almena_suite::digest::WIDTH
            || bytes.len() != 2 + almena_suite::digest::WIDTH
        {
            return Err(NotADid::Name);
        }
        Ok(Self(text.to_owned()))
    }
}

impl Did {
    /// The DID an object of this network gets from its creation.
    #[must_use]
    pub const fn new(network: Network, name: Name) -> Self {
        Self { network, name }
    }

    /// Which network it belongs to — **as written, which is a courtesy and not the test.**
    ///
    /// The network is inside the genesis operation and in what nodes tell each other on connecting,
    /// so a DID from another network fails to resolve whether or not anybody read the prefix. The
    /// mark is there so a person spots a test identifier where it does not belong — merging two
    /// networks is the accident that costs the most.
    #[must_use]
    pub const fn network(&self) -> Network {
        self.network
    }

    /// The name inside it.
    #[must_use]
    pub const fn name(&self) -> &Name {
        &self.name
    }

    /// A DID read back from how it was written.
    ///
    /// # Errors
    ///
    /// [`NotADid::Method`] when it is not a `did:almena`, [`NotADid::Name`] when the name is not
    /// one this method can produce.
    pub fn parse(text: &str) -> Result<Self, NotADid> {
        let rest = text.strip_prefix("did:almena:").ok_or(NotADid::Method)?;
        let (network, name) = rest
            .strip_prefix("dev:")
            .map_or((Network::Production, rest), |name| {
                (Network::Development, name)
            });
        Ok(Self::new(network, Name::parse(name)?))
    }
}

impl core::fmt::Display for Did {
    /// **Only development carries a mark, and the asymmetry is deliberate**: what has to
    /// be obvious at a glance is a test identifier where it does not belong, not the other way
    /// round.
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self.network {
            Network::Production => write!(f, "did:almena:{}", self.name.as_str()),
            Network::Development => write!(f, "did:almena:dev:{}", self.name.as_str()),
        }
    }
}

/// Bytes in base58btc, the alphabet everything here is named in.
///
/// Public because a node's name on the mesh is written in it too, and a second implementation of
/// one encoding is two things that can disagree about a name.
#[must_use]
pub fn base58(bytes: &[u8]) -> String {
    let mut digits: Vec<u8> = Vec::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        let mut carry = u32::from(byte);
        for digit in &mut digits {
            carry += u32::from(*digit) << 8;
            *digit = (carry % 58) as u8;
            carry /= 58;
        }
        while carry > 0 {
            digits.push((carry % 58) as u8);
            carry /= 58;
        }
    }

    // A leading zero byte carries no value and would vanish in the arithmetic, so it is written
    // out separately — which is why base58 has a digit that means nothing but position.
    let leading = bytes.iter().take_while(|&&byte| byte == 0).count();
    let mut out = "1".repeat(leading);
    out.extend(
        digits
            .iter()
            .rev()
            .map(|&d| char::from(ALPHABET[d as usize])),
    );
    out
}

/// Base58 back to bytes. [`None`] when a character is not one of the fifty-eight.
///
/// Public because it is the other half of [`base58`], and anything written in the alphabet
/// everything here is named in has to be readable by whoever has to read it back.
#[must_use]
pub fn unbase58(text: &str) -> Option<Vec<u8>> {
    let mut bytes: Vec<u8> = Vec::with_capacity(text.len());
    for character in text.bytes() {
        let mut carry = ALPHABET.iter().position(|&d| d == character)? as u32;
        for byte in &mut bytes {
            carry += u32::from(*byte) * 58;
            *byte = (carry & 0xff) as u8;
            carry >>= 8;
        }
        while carry > 0 {
            bytes.push((carry & 0xff) as u8);
            carry >>= 8;
        }
    }

    let leading = text.bytes().take_while(|&d| d == b'1').count();
    bytes.extend(core::iter::repeat_n(0, leading));
    bytes.reverse();
    Some(bytes)
}

#[cfg(test)]
mod tests {
    use super::{Did, Name, Network, NotADid, base58, unbase58};

    #[test]
    fn base58_matches_the_published_vectors() {
        // From the base58 test vectors everyone uses, including multibase's own.
        for (bytes, text) in [
            (&b""[..], ""),
            (&b"\x00"[..], "1"),
            (&b"\x00\x00abc"[..], "11ZiCa"),
            (&b"hello world"[..], "StV1DL6CwTryKyV"),
            (&b"\x61"[..], "2g"),
            (&b"\x62\x62\x62"[..], "a3gV"),
        ] {
            assert_eq!(base58(bytes), text, "encoding {bytes:?}");
            assert_eq!(unbase58(text).as_deref(), Some(bytes), "decoding {text:?}");
        }
    }

    #[test]
    fn a_name_says_which_hash_is_inside_it() {
        // Multihash: the SHA-256 code, the length, then the digest. Written into the name so that
        // changing the function some day is a version and not a new syntax.
        let name = Name::of(b"a creation operation");
        let decoded = unbase58(&name.as_str()[1..]).expect("base58");
        assert_eq!(decoded[0], 0x12, "sha2-256");
        assert_eq!(decoded[1], 32, "thirty-two bytes of it");
        assert_eq!(decoded.len(), 34);
        assert!(name.as_str().starts_with('z'), "base58btc, per multibase");
    }

    #[test]
    fn the_same_operation_always_gets_the_same_name() {
        assert_eq!(Name::of(b"an operation"), Name::of(b"an operation"));
        assert_ne!(Name::of(b"an operation"), Name::of(b"another operation"));
    }

    #[test]
    fn only_development_is_marked() {
        let name = Name::of(b"a creation operation");
        let production = Did::new(Network::Production, name.clone()).to_string();
        let development = Did::new(Network::Development, name.clone()).to_string();

        assert_eq!(production, format!("did:almena:{}", name.as_str()));
        assert_eq!(development, format!("did:almena:dev:{}", name.as_str()));
        assert!(!production.contains("dev"), "production goes unmarked");
    }

    #[test]
    fn a_did_survives_a_round_trip_through_how_it_is_written() {
        for network in [Network::Production, Network::Development] {
            let did = Did::new(network, Name::of(b"a creation operation"));
            assert_eq!(Did::parse(&did.to_string()), Ok(did));
        }
    }

    #[test]
    fn another_method_is_not_this_one() {
        assert_eq!(Did::parse("did:key:z6Mk"), Err(NotADid::Method));
        assert_eq!(Did::parse("not a did at all"), Err(NotADid::Method));
    }

    #[test]
    fn a_name_that_is_not_a_sha_256_multihash_is_refused() {
        // Right shape, wrong hash: 0x13 is SHA-512.
        let mut wrong = vec![0x13, 32];
        wrong.extend_from_slice(&[7; 32]);
        let text = format!("z{}", base58(&wrong));
        assert_eq!(Name::parse(&text), Err(NotADid::Name));
    }

    #[test]
    fn a_name_in_the_wrong_base_is_refused() {
        // Multibase says the first character is the base. 'f' is hex, and this method is base58btc.
        assert_eq!(Name::parse("f1220aabb"), Err(NotADid::Name));
    }
}
