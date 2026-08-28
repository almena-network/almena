//! Ed25519: the control key, node keys, and the seal if one is ever shared out.
//!
//! Chosen where the choice was free. The control key does not live in a hardware store — it
//! comes from the holder's words, and an enclave generates its own keys and imports nobody
//! else's — so nothing forced a curve, and the key hardest to replace is the one that should
//! admit the least room for error: a deterministic signature with no nonce to generate is one
//! fewer way to go wrong. A node signing a root every hour for years gets the same benefit.
//!
//! It is also what makes an entity's threshold seal possible at all: FROST over Ed25519
//! produces an ordinary Ed25519 signature, so whoever verifies one neither needs to know nor
//! can tell that it came from k fragments.

use ed25519_dalek::{Signer as _, Verifier as _};

/// How many bytes a public key takes.
pub const PUBLIC_KEY_WIDTH: usize = 32;

/// How many bytes a signature takes.
pub const SIGNATURE_WIDTH: usize = 64;

/// A key that can sign.
///
/// It never leaves the process that made it, and nothing here writes one to disk: where a
/// secret rests is the business of whichever program holds it — the encrypted store on the
/// device for the holder's app, and a node's own directory for a node.
pub struct SigningKey(ed25519_dalek::SigningKey);

/// A key that can check a signature.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerifyingKey(ed25519_dalek::VerifyingKey);

/// A signature over some bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Signature(ed25519_dalek::Signature);

/// Why some bytes were not a key or a signature.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Invalid {
    /// The bytes are not a point this curve has.
    Key,
    /// The signature does not check out against this key and these bytes.
    Signature,
}

impl SigningKey {
    /// A signing key from thirty-two bytes of secret.
    ///
    /// Where those bytes come from is the caller's business: `super::seed` derives the
    /// holder's control key from their words, and a node makes its own at random.
    #[must_use]
    pub fn from_secret(secret: [u8; PUBLIC_KEY_WIDTH]) -> Self {
        Self(ed25519_dalek::SigningKey::from_bytes(&secret))
    }

    /// The secret itself, for the one caller that has to hand it to something else.
    ///
    /// **Not for signing.** Everything that signs does it through [`SigningKey::sign`], which never
    /// lets the bytes out. This exists because a node's identity on the mesh is built by a library
    /// that wants the key rather than a signature, and the alternative would be a second key — and
    /// a second key is a second census of nodes to keep in step with the first.
    ///
    /// Whoever takes a copy owns the problem of getting rid of it.
    #[must_use]
    pub fn secret(&self) -> [u8; PUBLIC_KEY_WIDTH] {
        self.0.to_bytes()
    }

    /// The public half, which is what goes in a DID document.
    #[must_use]
    pub fn verifying_key(&self) -> VerifyingKey {
        VerifyingKey(self.0.verifying_key())
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
    /// [`Invalid::Key`] when the bytes are not a point on the curve.
    pub fn from_bytes(bytes: [u8; PUBLIC_KEY_WIDTH]) -> Result<Self, Invalid> {
        ed25519_dalek::VerifyingKey::from_bytes(&bytes)
            .map(Self)
            .map_err(|_| Invalid::Key)
    }

    /// The key as the bytes a DID document carries.
    #[must_use]
    pub fn bytes(&self) -> [u8; PUBLIC_KEY_WIDTH] {
        self.0.to_bytes()
    }

    /// Whether this signature is this key's, over these bytes.
    ///
    /// # Errors
    ///
    /// [`Invalid::Signature`] when it is not. There is no third answer: a signature either
    /// checks out or it does not, and an operation is admitted on that basis and no other.
    pub fn verify(&self, message: &[u8], signature: &Signature) -> Result<(), Invalid> {
        self.0
            .verify(message, &signature.0)
            .map_err(|_| Invalid::Signature)
    }
}

impl Signature {
    /// A signature read from the bytes an operation carries.
    #[must_use]
    pub fn from_bytes(bytes: [u8; SIGNATURE_WIDTH]) -> Self {
        Self(ed25519_dalek::Signature::from_bytes(&bytes))
    }

    /// The signature as the bytes an operation carries.
    #[must_use]
    pub fn bytes(&self) -> [u8; SIGNATURE_WIDTH] {
        self.0.to_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::{Invalid, Signature, SigningKey, VerifyingKey};

    fn key() -> SigningKey {
        SigningKey::from_secret([7; 32])
    }

    #[test]
    fn a_key_signs_and_its_public_half_checks() {
        let signing = key();
        let signature = signing.sign(b"an operation");
        assert_eq!(
            signing.verifying_key().verify(b"an operation", &signature),
            Ok(())
        );
    }

    #[test]
    fn another_message_does_not_check() {
        let signing = key();
        let signature = signing.sign(b"an operation");
        assert_eq!(
            signing
                .verifying_key()
                .verify(b"another operation", &signature),
            Err(Invalid::Signature)
        );
    }

    #[test]
    fn another_key_does_not_check() {
        let signature = key().sign(b"an operation");
        let stranger = SigningKey::from_secret([9; 32]);
        assert_eq!(
            stranger.verifying_key().verify(b"an operation", &signature),
            Err(Invalid::Signature)
        );
    }

    #[test]
    fn the_signature_is_deterministic() {
        // The property the curve was chosen for: no nonce, so no nonce to get wrong, and the
        // same operation signed twice is the same bytes twice.
        assert_eq!(
            key().sign(b"an operation").bytes(),
            key().sign(b"an operation").bytes()
        );
    }

    #[test]
    fn keys_and_signatures_survive_a_round_trip_through_their_bytes() {
        let signing = key();
        let public = signing.verifying_key();
        assert_eq!(VerifyingKey::from_bytes(public.bytes()), Ok(public));

        let signature = signing.sign(b"an operation");
        let read = Signature::from_bytes(signature.bytes());
        assert_eq!(public.verify(b"an operation", &read), Ok(()));
    }

    #[test]
    fn bytes_that_are_not_a_point_are_refused() {
        // y = 2, which no point on this curve has: nothing squares to the x it would need.
        let mut not_a_point = [0; 32];
        not_a_point[0] = 2;
        assert_eq!(VerifyingKey::from_bytes(not_a_point), Err(Invalid::Key));
    }
}
