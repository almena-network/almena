//! From the holder's words to the key that governs their account.
//!
//! The root of a person's identity is **twelve words by default, twenty-four as an option** —
//! BIP39, and twelve rather than twenty-four because a hundred and twenty-eight bits are already
//! plenty and being asked for a few of them back every so often is far less punishing with the
//! shorter phrase. The control key comes from them by **SLIP-0010**. This module is that path,
//! and nothing else: it turns words into a seed, and a seed into keys.
//!
//! **Two keys come out, by different paths.** The control key, which governs the devices and the
//! account itself and never signs day-to-day traffic; and the key that encrypts the backup before
//! it reaches iCloud or Android, on a path of its own precisely so that a backup that leaks does
//! not hand over the account as well.
//!
//! Device keys are **not** here and never will be. They are born inside the enclave, do not derive
//! from the seed and do not travel — which is the whole reason typing the words on a new phone is
//! an enrolment and not a restore.

use hmac::digest::KeyInit;
use hmac::{Hmac, Mac as _};
use sha2::Sha512;

/// The path index of the control key, the one that governs the devices and the account.
const CONTROL: u32 = 0;

/// The path index of the key the backup is encrypted under before the system takes it.
const BACKUP: u32 = 1;

/// The string SLIP-0010 hashes the seed under to reach the master key for this curve.
const MASTER: &[u8] = b"ed25519 seed";

/// How many bytes a BIP39 seed takes.
pub const WIDTH: usize = 64;

/// The seed behind an account: what the words mean, before any key is derived from them.
///
/// Holding one is holding the account. Nothing here writes it anywhere; where it rests — and
/// whether it rests at all — is the business of the app that holds it.
pub struct Seed([u8; WIDTH]);

/// Why some words were not a phrase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotAPhrase {
    /// Not a count BIP39 admits, or a word that is not in the list, or a checksum that fails.
    ///
    /// One answer for all three on purpose: the person is told *"those words do not open an
    /// account"* and never which word was wrong, because whoever is typing a phrase they did not
    /// write is exactly who a more helpful message would help.
    Invalid,
}

impl Seed {
    /// The seed a phrase means.
    ///
    /// The empty passphrase is deliberate. BIP39 admits a thirteenth secret, and Almena does not
    /// take it: a person who loses it loses the account with the words still in hand, and social
    /// recovery exists so that the answer to a lost account is guardians rather than a second
    /// thing to lose.
    ///
    /// # Errors
    ///
    /// [`NotAPhrase::Invalid`] when the words are not a BIP39 phrase.
    pub fn from_words(words: &str) -> Result<Self, NotAPhrase> {
        bip39::Mnemonic::parse_in_normalized(bip39::Language::English, words)
            .map(|phrase| Self(phrase.to_seed_normalized("")))
            .map_err(|_| NotAPhrase::Invalid)
    }

    /// A seed straight from its bytes, for a caller that already has one.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; WIDTH]) -> Self {
        Self(bytes)
    }

    /// The control key: it governs the devices and the account, and signs nothing else.
    #[must_use]
    pub fn control_key(&self) -> super::ed25519::SigningKey {
        super::ed25519::SigningKey::from_secret(self.derive(CONTROL))
    }

    /// The key the backup is encrypted under before the operating system takes it.
    ///
    /// Bytes rather than a key type: this one does not sign, it encrypts, and which cipher uses
    /// it belongs to whatever writes the backup, not to this module.
    #[must_use]
    pub fn backup_key(&self) -> [u8; 32] {
        self.derive(BACKUP)
    }

    /// One hardened step down from the master key, which is all the depth this platform uses.
    ///
    /// SLIP-0010 over this curve admits **only** hardened derivation — there is no public
    /// derivation to lose by it — and Almena derives exactly two things from a seed. A coin-type
    /// level borrowed from BIP-44 would be ceremony pointing at a registry Almena is not in.
    fn derive(&self, index: u32) -> [u8; 32] {
        derive_from(&self.0, index)
    }
}

/// The derivation itself, over any seed: master key, then one hardened step.
///
/// Separate from [`Seed`] only so that SLIP-0010's own published vectors — whose seeds are not
/// BIP39 seeds and are not sixty-four bytes — can drive this exact code rather than a copy of it.
fn derive_from(seed: &[u8], index: u32) -> [u8; 32] {
    let master = hmac(MASTER, seed);
    let (key, chain_code) = master.split_at(32);

    let mut step = [0; 37];
    step[1..33].copy_from_slice(key);
    step[33..].copy_from_slice(&(index | 0x8000_0000).to_be_bytes());

    let child = hmac(chain_code, &step);
    let mut secret = [0; 32];
    secret.copy_from_slice(&child[..32]);
    secret
}

/// How many bytes SHA-512 takes at a time, and therefore how long an HMAC key is.
const BLOCK: usize = 128;

/// HMAC-SHA512, which is the only primitive SLIP-0010 needs.
///
/// The key arrives already padded to the block, which is what HMAC does to a short key anyway.
/// Handing it over that way is not a shortcut: it is the difference between a function that
/// cannot fail and one that returns an error nobody can trigger and every caller has to pretend
/// to handle. Both keys this module uses — the twelve-byte string below and a chain code — are
/// far shorter than a block, and the assertion says so once rather than hoping.
fn hmac(key: &[u8], data: &[u8]) -> [u8; 64] {
    debug_assert!(key.len() <= BLOCK, "SLIP-0010 never keys HMAC past a block");
    let mut padded = [0; BLOCK];
    padded[..key.len()].copy_from_slice(key);

    let mut mac = <Hmac<Sha512> as KeyInit>::new(&padded.into());
    mac.update(data);
    mac.finalize().into_bytes().into()
}

#[cfg(test)]
mod tests {
    use super::{MASTER, NotAPhrase, Seed};

    /// The twelve words of SLIP-0039's and BIP39's own examples, valid checksum and all.
    const PHRASE: &str = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    #[test]
    fn slip_0010_test_vector_one() {
        // The published seed of SLIP-0010's first ed25519 vector, its master key and its m/0' —
        // which is the control key. The vector publishes no m/1', so the backup key is held to
        // the same code rather than to a second vector. Ours is the same derivation as everyone
        // else's, or the same words open a different account elsewhere.
        let seed: [u8; 16] = [
            0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d,
            0x0e, 0x0f,
        ];

        let master = super::hmac(MASTER, &seed);
        assert_eq!(
            hex(&master[..32]),
            "2b4be7f19ee27bbf30c667b642d5f4aa69fd169872f8fc3059c08ebae2eb19e7",
            "master private key"
        );
        assert_eq!(
            hex(&master[32..]),
            "90046a93de5380a72b5e45010748567d5ea02bbf6522f979e05c0d8d8ca9fffb",
            "master chain code"
        );
        assert_eq!(
            hex(&super::derive_from(&seed, super::CONTROL)),
            "68e0fe46dfb67e368c75379acec591dad19df3cde26e63b93a8e704f1dade7a3",
            "m/0'"
        );
    }

    #[test]
    fn the_two_keys_are_not_the_same_key() {
        // What the separate path is for: a leaked backup key must not be the key that governs
        // the account.
        let seed = Seed::from_words(PHRASE).expect("a valid phrase");
        let control = super::derive_from(&seed.0, super::CONTROL);
        assert_ne!(control, seed.backup_key());
    }

    #[test]
    fn the_same_words_always_give_the_same_account() {
        let first = Seed::from_words(PHRASE).expect("a valid phrase");
        let again = Seed::from_words(PHRASE).expect("a valid phrase");
        assert_eq!(
            first.control_key().verifying_key().bytes(),
            again.control_key().verifying_key().bytes()
        );
    }

    #[test]
    fn spacing_does_not_change_the_account() {
        // What a person types has stray whitespace in it. It must not silently open a different
        // account — that failure is invisible, and no screen has a way to show it.
        let spaced = Seed::from_words(&PHRASE.replace(' ', "  ")).expect("a valid phrase");
        let plain = Seed::from_words(PHRASE).expect("a valid phrase");
        assert_eq!(
            spaced.control_key().verifying_key().bytes(),
            plain.control_key().verifying_key().bytes()
        );
    }

    #[test]
    fn a_phrase_whose_checksum_fails_is_refused() {
        let wrong = PHRASE.replace("about", "abandon");
        assert_eq!(Seed::from_words(&wrong).err(), Some(NotAPhrase::Invalid));
    }

    #[test]
    fn too_few_words_are_refused() {
        assert_eq!(
            Seed::from_words("abandon abandon about").err(),
            Some(NotAPhrase::Invalid)
        );
    }
}
