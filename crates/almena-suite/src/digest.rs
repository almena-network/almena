//! SHA-256, which is the only hash this platform has.
//!
//! Everything named is named by its digest, so this is the function the whole scheme of identity
//! hangs from. It is SHA-256 because that is what Certificate Transparency uses — the model this
//! log follows — what pairs with P-256 in COSE and JOSE, and what every hardware key store
//! implements without exception.
//!
//! Changing it one day is a protocol version and not a rewrite: an identifier carries its hash
//! function inside, as a multihash, so old names keep meaning what they meant.

use sha2::{Digest as _, Sha256};

/// The width of a SHA-256 digest, in bytes.
pub const WIDTH: usize = 32;

/// A SHA-256 digest.
///
/// A type of its own rather than a bare array, so that a digest cannot be passed where any
/// thirty-two bytes would do — which in this codebase is most places.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Digest([u8; WIDTH]);

impl Digest {
    /// The digest of these bytes.
    #[must_use]
    pub fn of(bytes: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        Self(hasher.finalize().into())
    }

    /// A digest read from bytes that already are one.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; WIDTH]) -> Self {
        Self(bytes)
    }

    /// The digest as bytes.
    #[must_use]
    pub const fn bytes(&self) -> &[u8; WIDTH] {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::{Digest, WIDTH};

    #[test]
    fn the_empty_input_hashes_to_what_the_standard_says() {
        // FIPS 180-4's own vector, so this test fails if the crate underneath is ever swapped
        // for one that is not SHA-256 at all.
        let expected = [
            0xe3, 0xb0, 0xc4, 0x42, 0x98, 0xfc, 0x1c, 0x14, 0x9a, 0xfb, 0xf4, 0xc8, 0x99, 0x6f,
            0xb9, 0x24, 0x27, 0xae, 0x41, 0xe4, 0x64, 0x9b, 0x93, 0x4c, 0xa4, 0x95, 0x99, 0x1b,
            0x78, 0x52, 0xb8, 0x55,
        ];
        assert_eq!(Digest::of(b"").bytes(), &expected);
    }

    #[test]
    fn abc_hashes_to_what_the_standard_says() {
        let expected = [
            0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40, 0xde, 0x5d, 0xae,
            0x22, 0x23, 0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c, 0xb4, 0x10, 0xff, 0x61,
            0xf2, 0x00, 0x15, 0xad,
        ];
        assert_eq!(Digest::of(b"abc").bytes(), &expected);
    }

    #[test]
    fn a_digest_survives_a_round_trip_through_its_bytes() {
        let digest = Digest::of(b"almena");
        assert_eq!(Digest::from_bytes(*digest.bytes()), digest);
        assert_eq!(digest.bytes().len(), WIDTH);
    }
}
