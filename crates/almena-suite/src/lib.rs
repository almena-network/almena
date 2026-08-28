//! The one set of algorithms every Almena program signs and hashes with.
//!
//! One set for the whole platform, gathered here rather than scattered: if each component chose
//! for itself, two honest implementations would stop agreeing — and here disagreeing means
//! computing a different name for the same object, since everything is named by the digest of
//! the operation that created it.
//!
//! # Two planes, and the rule that separates them
//!
//! > **P-256 where the hardware decides and where the ecosystem decides; Ed25519 where the
//! > cryptography decides.**
//!
//! - **[`ed25519`]** — the holder's control key, node keys, and the entity seal when one is
//!   ever shared out. Deterministic signatures with no nonce to get wrong, and the one
//!   threshold scheme whose output is an ordinary signature is built on it.
//! - **[`p256`]** — device keys and issuance. **Not a decision**: a Secure Enclave does P-256
//!   and nothing else, and ES256 is what the EUDI ARF and ISO 18013-5 expect of a credential.
//! - **[`digest`]** — SHA-256, everywhere. What Certificate Transparency uses, what pairs with
//!   P-256 in COSE and JOSE, and present in every hardware store without exception.
//! - **[`seed`]** — BIP39 words to the control key, through SLIP-0010.
//!
//! # What is not here
//!
//! **Threshold signing.** FROST is for the seal on what leaves the registry — documents,
//! invoices, certificates — and even that waits on a legal review of what sealing means, which
//! nobody has commissioned yet. Nothing in the registry needs it: operations there carry counted
//! individual signatures.
//!
//! **Encryption.** DIDComm's ECDH lives with the messaging that uses it, not here.

pub mod digest;
pub mod ed25519;
pub mod p256;
pub mod seed;
