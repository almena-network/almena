//! P-256: device keys, and the key an issuer signs credentials with.
//!
//! Not chosen — imposed twice over. The Secure Enclave does P-256 and nothing else, and
//! StrongBox and the TPM go the same way, so a device key has no other option. And ES256 is what
//! the EUDI ARF and ISO 18013-5 ask of a credential, the two sources the attribute vocabulary
//! leans on: a credential in EdDSA would need translating at the bridge left open to the European
//! wallet precisely so that crossing it would be an extension and not a migration.
//!
//! **There is no randomness in this crate.** Signing here derives its nonce from the message
//! (RFC 6979), and key generation is not offered at all: a device key is born inside the enclave,
//! and an issuance key is born in the process that will hold it. Whoever needs a new key brings
//! the bytes. That keeps every path here reproducible in a test and leaves no room for a weak
//! generator to weaken anything.
//!
//! **But nothing may depend on a signature being reproducible.** Ours are; an enclave's are not,
//! and most of the P-256 signatures this platform verifies will come from one. That asymmetry is
//! why an object's identifier is computed over the operation *without* its signatures — if it
//! included them, the same act could end up with two names.

use p256::ecdsa::signature::{Signer as _, Verifier as _};

/// How many bytes a public key takes, in the compressed form (`SEC1`) a DID document carries.
pub const PUBLIC_KEY_WIDTH: usize = 33;

/// How many bytes a signature takes: `r` then `s`, fixed width, as `ES256` writes them.
///
/// Not DER. A JWS carries the pair raw, and every credential here is an SD-JWT VC.
pub const SIGNATURE_WIDTH: usize = 64;

/// A key that can sign.
pub struct SigningKey(p256::ecdsa::SigningKey);

/// A key that can check a signature.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerifyingKey(p256::ecdsa::VerifyingKey);

/// A signature over some bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Signature(p256::ecdsa::Signature);

/// Why some bytes were not a key or a signature.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Invalid {
    /// The bytes are not a usable scalar, or not a point this curve has.
    Key,
    /// The bytes are not a well-formed `(r, s)`, or the signature does not check out.
    Signature,
}

impl SigningKey {
    /// A signing key from thirty-two bytes of secret.
    ///
    /// # Errors
    ///
    /// [`Invalid::Key`] when the bytes are not a scalar this curve can use — zero, or past the
    /// order of the group.
    pub fn from_secret(secret: [u8; 32]) -> Result<Self, Invalid> {
        p256::ecdsa::SigningKey::from_bytes(&secret.into())
            .map(Self)
            .map_err(|_| Invalid::Key)
    }

    /// The public half, which is what goes in a DID document.
    #[must_use]
    pub fn verifying_key(&self) -> VerifyingKey {
        VerifyingKey(*self.0.verifying_key())
    }

    /// This key's signature over these bytes.
    #[must_use]
    pub fn sign(&self, message: &[u8]) -> Signature {
        Signature(self.0.sign(message))
    }
}

impl VerifyingKey {
    /// A verifying key read from the bytes a DID document carries.
    ///
    /// # Errors
    ///
    /// [`Invalid::Key`] when the bytes are not a point on the curve, or are not the compressed
    /// form: an uncompressed point is the right key written the wrong way, and everything gets
    /// one encoding and no second one, so that one object cannot have two names.
    pub fn from_bytes(bytes: [u8; PUBLIC_KEY_WIDTH]) -> Result<Self, Invalid> {
        p256::ecdsa::VerifyingKey::from_sec1_bytes(&bytes)
            .map(Self)
            .map_err(|_| Invalid::Key)
    }

    /// The key as the bytes a DID document carries.
    #[must_use]
    pub fn bytes(&self) -> [u8; PUBLIC_KEY_WIDTH] {
        let point = self.0.to_encoded_point(true);
        let mut bytes = [0; PUBLIC_KEY_WIDTH];
        bytes.copy_from_slice(point.as_bytes());
        bytes
    }

    /// The point's two coordinates, which is how a JWK writes a key.
    ///
    /// **Here rather than wherever a JWK is built**, because this crate is the one place the curve
    /// is touched: a second decompression somewhere else would be a second opinion about what a
    /// key is, and the day the two disagreed a credential would name a holder nobody can find.
    ///
    /// Thirty-two bytes each, big-endian and left-padded, which is what `crv: P-256` fixes.
    #[must_use]
    pub fn coordinates(&self) -> ([u8; 32], [u8; 32]) {
        let point = self.0.to_encoded_point(false);
        let mut x = [0; 32];
        let mut y = [0; 32];
        // Uncompressed is `0x04 ‖ x ‖ y`, and this is the library's own encoding of a point it
        // already holds — there is nothing here that could be short.
        x.copy_from_slice(point.x().map_or(&[0; 32][..], |held| held.as_slice()));
        y.copy_from_slice(point.y().map_or(&[0; 32][..], |held| held.as_slice()));
        (x, y)
    }

    /// Whether this signature is this key's, over these bytes.
    ///
    /// # Errors
    ///
    /// [`Invalid::Signature`] when it is not.
    pub fn verify(&self, message: &[u8], signature: &Signature) -> Result<(), Invalid> {
        self.0
            .verify(message, &signature.0)
            .map_err(|_| Invalid::Signature)
    }
}

impl Signature {
    /// A signature read from the sixty-four bytes an operation or a JWS carries.
    ///
    /// **Both halves of the pair are accepted.** An ECDSA signature can be written two ways —
    /// `s` and the order minus `s` — and it is tempting to insist on one, since everything else
    /// here gets a single encoding. Doing so would refuse the signatures of the Secure
    /// Enclave, StrongBox and the TPM, which normalise nothing, along with a good half of every
    /// ES256 signature in the wild. It would refuse, in other words, the hardware this curve was
    /// picked for.
    ///
    /// Nothing is lost by accepting both, because no name depends on a signature: an object's
    /// identifier is computed over the operation *without* its signatures, so the same act
    /// rewritten with the other half is the same object and not a second one.
    ///
    /// # Errors
    ///
    /// [`Invalid::Signature`] when `r` or `s` is zero or past the order of the group.
    pub fn from_bytes(bytes: [u8; SIGNATURE_WIDTH]) -> Result<Self, Invalid> {
        p256::ecdsa::Signature::from_slice(&bytes)
            .map(Self)
            .map_err(|_| Invalid::Signature)
    }

    /// The signature as `r` then `s`, the sixty-four bytes an operation or a JWS carries.
    #[must_use]
    pub fn bytes(&self) -> [u8; SIGNATURE_WIDTH] {
        self.0.to_bytes().into()
    }
}

#[cfg(test)]
mod tests {
    use super::{Invalid, SIGNATURE_WIDTH, Signature, SigningKey, VerifyingKey};

    fn key() -> SigningKey {
        SigningKey::from_secret([7; 32]).expect("a valid scalar")
    }

    #[test]
    fn a_key_signs_and_its_public_half_checks() {
        let signing = key();
        let signature = signing.sign(b"a credential");
        assert_eq!(
            signing.verifying_key().verify(b"a credential", &signature),
            Ok(())
        );
    }

    #[test]
    fn another_message_does_not_check() {
        let signing = key();
        let signature = signing.sign(b"a credential");
        assert_eq!(
            signing
                .verifying_key()
                .verify(b"another credential", &signature),
            Err(Invalid::Signature)
        );
    }

    #[test]
    fn another_key_does_not_check() {
        let signature = key().sign(b"a credential");
        let stranger = SigningKey::from_secret([9; 32]).expect("a valid scalar");
        assert_eq!(
            stranger.verifying_key().verify(b"a credential", &signature),
            Err(Invalid::Signature)
        );
    }

    #[test]
    fn keys_and_signatures_survive_a_round_trip_through_their_bytes() {
        let signing = key();
        let public = signing.verifying_key();
        assert_eq!(VerifyingKey::from_bytes(public.bytes()), Ok(public));

        let signature = signing.sign(b"a credential");
        let read = Signature::from_bytes(signature.bytes()).expect("a signature we just made");
        assert_eq!(public.verify(b"a credential", &read), Ok(()));
    }

    #[test]
    fn a_public_key_is_thirty_three_bytes_and_says_which_half_it_is() {
        // The compressed form: a parity byte and the x coordinate. Anything else is the same key
        // written a second way, which is what one encoding per thing does not allow.
        let bytes = key().verifying_key().bytes();
        assert!(
            bytes[0] == 2 || bytes[0] == 3,
            "leading byte was {}",
            bytes[0]
        );
    }

    #[test]
    fn zero_is_not_a_key() {
        assert_eq!(SigningKey::from_secret([0; 32]).err(), Some(Invalid::Key));
    }

    #[test]
    fn every_signature_is_readable_back_whichever_half_it_used() {
        // Half of these come out with a high `s`. An enclave normalises nothing and neither do
        // we, so all twenty must survive the round trip — this test is here to fail the day
        // somebody adds a low-`s` rule and quietly refuses the hardware this curve was picked for.
        let signing = key();
        let public = signing.verifying_key();
        for n in 0..20u8 {
            let message = [n; 4];
            let signature = signing.sign(&message);
            let read = Signature::from_bytes(signature.bytes()).expect("a signature we just made");
            assert_eq!(public.verify(&message, &read), Ok(()), "message {n}");
        }
    }

    #[test]
    fn a_signature_that_is_not_a_pair_of_scalars_is_refused() {
        assert_eq!(
            Signature::from_bytes([0; SIGNATURE_WIDTH]),
            Err(Invalid::Signature)
        );
    }
}
