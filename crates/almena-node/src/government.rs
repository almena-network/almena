//! Almena Government's key, kept beside the record of the node that opened the network.
//!
//! **The key belongs to the network and is made with it.** Opening a network signs the genesis with
//! a fresh key, and that key is the trust anchor everything on the network is checked against
//! until Almena Government has owners of its own (`SPECS.md §7.9`). It is not the node's key: the
//! node signs roots and answers to the mesh with its own, and a node that has joined rather than
//! opened never holds this one at all.
//!
//! # Why it is written down at all
//!
//! Without it the network would open and the key would be gone with the process — and the core the
//! whole catalogue references, the first certifications and every answer to a request would have
//! nobody able to sign them. A first version needs somebody able to act as the government, and the
//! only moment the key exists is the moment the network opens. So the node that opens a network
//! keeps the key in its own directory, readable by its owner alone, and says where in its records.
//!
//! # It is never made here and never replaced
//!
//! There is one place a government key comes from, which is opening a network; this only keeps it
//! and reads it back. A file that is there and unreadable stops the reading and is not written
//! over: whoever holds the directory has to decide what it is, and deciding for them would be
//! silently becoming somebody else's government.

use std::fs;
use std::path::{Path, PathBuf};

use almena_suite::ed25519;

/// What the key is kept in, inside the node's own directory.
const FILE: &str = "government.key";

/// Why a directory could not hand over Almena Government's key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoKey {
    /// There is no such file: this node did not open the network, or the key was taken elsewhere.
    NotHere,
    /// There is a file and it is not a key.
    ///
    /// **Nothing is written in this case.** A directory whose government key cannot be read is a
    /// question for whoever owns it, not something to resolve by making another.
    Unreadable,
    /// The directory could not be written into, or the file is already there.
    ///
    /// Already there is refused too: a government key is written once, when the network opens,
    /// and a second write would be a second network's key over the first's.
    NotWritable,
}

/// Where the government key of the network opened from `directory` lives.
#[must_use]
pub fn at(directory: &Path) -> PathBuf {
    directory.join(FILE)
}

/// Keep the key a network was just opened with, readable by the owner of the directory alone.
///
/// The permissions are set **as the file is created**, not afterwards, for the reason the node's
/// own identity is: a file that was briefly world-readable was readable.
///
/// # Errors
///
/// [`NoKey::NotWritable`] when the directory will not take the file, or already holds one.
pub fn keep(directory: &Path, key: &ed25519::SigningKey) -> Result<PathBuf, NoKey> {
    let path = at(directory);
    fs::create_dir_all(directory).map_err(|_| NoKey::NotWritable)?;
    write_privately(&path, &key.secret())?;
    Ok(path)
}

/// The government key kept in `directory`, if the network was opened from there.
///
/// # Errors
///
/// [`NoKey`], telling apart a directory that never held one from one whose file is not a key.
pub fn load(directory: &Path) -> Result<ed25519::SigningKey, NoKey> {
    match fs::read(at(directory)) {
        Ok(bytes) => {
            let secret: [u8; ed25519::PUBLIC_KEY_WIDTH] =
                bytes.as_slice().try_into().map_err(|_| NoKey::Unreadable)?;
            Ok(ed25519::SigningKey::from_secret(secret))
        }
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => Err(NoKey::NotHere),
        Err(_) => Err(NoKey::Unreadable),
    }
}

/// Write the key so that only its owner can read it, and only where there is none yet.
#[cfg(unix)]
fn write_privately(path: &Path, secret: &[u8]) -> Result<(), NoKey> {
    use std::io::Write as _;
    use std::os::unix::fs::OpenOptionsExt as _;

    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(|_| NoKey::NotWritable)?;
    file.write_all(secret).map_err(|_| NoKey::NotWritable)
}

/// The same, where the platform has no such thing as a mode.
///
/// Windows inherits the directory's own protection, which for a per-user application directory is
/// what this wants anyway.
#[cfg(not(unix))]
fn write_privately(path: &Path, secret: &[u8]) -> Result<(), NoKey> {
    use std::io::Write as _;

    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|_| NoKey::NotWritable)?;
    file.write_all(secret).map_err(|_| NoKey::NotWritable)
}

#[cfg(test)]
mod tests {
    use super::{NoKey, at, keep, load};
    use almena_suite::ed25519;

    /// A directory of this test's own, removed when it is done with it.
    struct Scratch(std::path::PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!("almena-government-{name}"));
            let _ = std::fs::remove_dir_all(&path);
            Self(path)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn key(seed: u8) -> ed25519::SigningKey {
        ed25519::SigningKey::from_secret([seed; 32])
    }

    #[test]
    fn the_key_a_network_was_opened_with_comes_back_from_the_directory() {
        let scratch = Scratch::new("kept");
        let path = keep(&scratch.0, &key(5)).expect("kept");
        assert_eq!(path, at(&scratch.0));
        assert_eq!(
            load(&scratch.0).expect("read back").secret(),
            key(5).secret(),
            "the same key, and not one made afresh"
        );
    }

    #[cfg(unix)]
    #[test]
    fn the_key_is_readable_by_its_owner_alone() {
        use std::os::unix::fs::PermissionsExt as _;
        let scratch = Scratch::new("private");
        let path = keep(&scratch.0, &key(5)).expect("kept");
        let mode = std::fs::metadata(path)
            .expect("metadata")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600, "{mode:o}");
    }

    #[test]
    fn a_directory_that_never_opened_a_network_has_no_government_key() {
        // Told apart from an unreadable one: this is a node that joined, which is the ordinary
        // state of every node but the first.
        let scratch = Scratch::new("joined");
        assert_eq!(load(&scratch.0).err(), Some(NoKey::NotHere));
    }

    #[test]
    fn a_file_that_is_not_a_key_is_neither_read_nor_written_over() {
        let scratch = Scratch::new("not-a-key");
        std::fs::create_dir_all(&scratch.0).expect("a directory");
        std::fs::write(at(&scratch.0), b"three bytes").expect("written");
        assert_eq!(load(&scratch.0).err(), Some(NoKey::Unreadable));
        assert_eq!(
            keep(&scratch.0, &key(5)),
            Err(NoKey::NotWritable),
            "and it is not written over either: one network, one key"
        );
        assert_eq!(
            std::fs::read(at(&scratch.0)).expect("still there"),
            b"three bytes"
        );
    }
}
