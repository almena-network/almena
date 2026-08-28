//! The key that makes a directory a node.
//!
//! **A node is a directory with a key in it.** Which program runs on top is a question of what
//! screen there is; two directories are two nodes, and the same directory is the same node whoever
//! starts it and however many times.
//!
//! That is what makes an identity somebody can publish. A key made afresh on every start would be
//! a different node every time — a new identity in the mesh, a stale record in the zone, and no way
//! for whoever wrote that record to know it had gone stale.
//!
//! # It is never replaced by accident
//!
//! A file that is there and unreadable **stops this**, and does not get written over. Overwriting
//! would not be recovering from an error: it would be silently becoming a different node, losing
//! whatever the old identity was known for, and doing it at exactly the moment somebody was already
//! confused about the state of the directory.
//!
//! Replacing an identity on purpose is deleting the file, which is a thing somebody does knowingly.

use std::fs;
use std::path::{Path, PathBuf};

use almena_suite::ed25519;

/// What the key is kept in, inside the node's own directory.
const FILE: &str = "identity.key";

/// Why a directory could not produce an identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoIdentity {
    /// There is a key file and it cannot be read, or it is not a key.
    ///
    /// **Nothing is written in this case.** A directory whose identity cannot be read is a
    /// question for whoever owns it, not something to resolve by quietly becoming somebody else.
    Unreadable,
    /// The directory could not be made, or the key could not be written into it.
    NotWritable,
    /// The operating system would not produce randomness, so there is no key to be.
    ///
    /// A node with a guessable key is worse than one that never came up, because it comes up.
    NoRandomness,
}

/// Where the key of a node in `directory` lives.
#[must_use]
pub fn at(directory: &Path) -> PathBuf {
    directory.join(FILE)
}

/// The identity of the node in `directory`, making one the first time and never again.
///
/// # Errors
///
/// [`NoIdentity`], telling apart a directory that cannot be written from one whose key cannot be
/// read — because the second is somebody's data and the first is only a permission.
pub fn load_or_make(directory: &Path) -> Result<ed25519::SigningKey, NoIdentity> {
    let path = at(directory);

    match fs::read(&path) {
        Ok(bytes) => {
            let secret: [u8; ed25519::PUBLIC_KEY_WIDTH] = bytes
                .as_slice()
                .try_into()
                .map_err(|_| NoIdentity::Unreadable)?;
            Ok(ed25519::SigningKey::from_secret(secret))
        }
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => make(directory, &path),
        // There, and not readable. Not a reason to write a new one over it.
        Err(_) => Err(NoIdentity::Unreadable),
    }
}

/// A key for a directory that has none yet.
fn make(directory: &Path, path: &Path) -> Result<ed25519::SigningKey, NoIdentity> {
    let mut secret = [0u8; ed25519::PUBLIC_KEY_WIDTH];
    getrandom::fill(&mut secret).map_err(|_| NoIdentity::NoRandomness)?;

    fs::create_dir_all(directory).map_err(|_| NoIdentity::NotWritable)?;
    write_privately(path, &secret)?;

    Ok(ed25519::SigningKey::from_secret(secret))
}

/// Write the key so that only its owner can read it.
///
/// The permissions are set **as the file is created**, not afterwards: a file that is briefly
/// world-readable is a file that was readable, and on a shared machine that moment is enough.
#[cfg(unix)]
fn write_privately(path: &Path, secret: &[u8]) -> Result<(), NoIdentity> {
    use std::io::Write as _;
    use std::os::unix::fs::OpenOptionsExt as _;

    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(|_| NoIdentity::NotWritable)?;
    file.write_all(secret).map_err(|_| NoIdentity::NotWritable)
}

/// The same, where the platform has no such thing as a mode.
///
/// Windows inherits the directory's own protection, which for a per-user application directory is
/// what this wants anyway.
#[cfg(not(unix))]
fn write_privately(path: &Path, secret: &[u8]) -> Result<(), NoIdentity> {
    use std::io::Write as _;

    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|_| NoIdentity::NotWritable)?;
    file.write_all(secret).map_err(|_| NoIdentity::NotWritable)
}

#[cfg(test)]
mod tests {
    use super::{NoIdentity, at, load_or_make};
    use std::path::PathBuf;

    /// A directory of this test's own, removed when it is done with it.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!("almena-identity-{name}"));
            let _ = std::fs::remove_dir_all(&path);
            Self(path)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn the_same_directory_is_the_same_node_however_many_times_it_starts() {
        // The whole point. A key made afresh on every start would be a different node every time,
        // and anything published about it would go stale without anybody being told.
        let scratch = Scratch::new("stable");

        let first = load_or_make(&scratch.0).expect("a key");
        for _ in 0..5 {
            let again = load_or_make(&scratch.0).expect("the same key");
            assert_eq!(again.verifying_key().bytes(), first.verifying_key().bytes());
        }
    }

    #[test]
    fn two_directories_are_two_nodes() {
        let one = Scratch::new("one");
        let other = Scratch::new("other");

        assert_ne!(
            load_or_make(&one.0).expect("a key").verifying_key().bytes(),
            load_or_make(&other.0)
                .expect("a key")
                .verifying_key()
                .bytes()
        );
    }

    #[test]
    fn a_key_that_cannot_be_read_stops_this_and_is_not_written_over() {
        // Overwriting would not be recovering from an error. It would be silently becoming a
        // different node, at exactly the moment somebody was already confused about the directory.
        let scratch = Scratch::new("unreadable");
        std::fs::create_dir_all(&scratch.0).expect("the directory");
        std::fs::write(at(&scratch.0), b"this is not a key").expect("the file");

        assert_eq!(load_or_make(&scratch.0).err(), Some(NoIdentity::Unreadable));
        assert_eq!(
            std::fs::read(at(&scratch.0)).expect("still there"),
            b"this is not a key",
            "and what was there is still there"
        );
    }

    #[test]
    fn a_directory_that_does_not_exist_yet_gets_made() {
        let scratch = Scratch::new("fresh");
        let deeper = scratch.0.join("nested").join("further");
        assert!(load_or_make(&deeper).is_ok());
        assert!(at(&deeper).exists());
    }

    #[cfg(unix)]
    #[test]
    fn nobody_else_can_read_it() {
        use std::os::unix::fs::PermissionsExt as _;

        let scratch = Scratch::new("private");
        load_or_make(&scratch.0).expect("a key");

        let mode = std::fs::metadata(at(&scratch.0))
            .expect("the file")
            .permissions()
            .mode();
        assert_eq!(
            mode & 0o777,
            0o600,
            "readable and writable by its owner only"
        );
    }

    #[test]
    fn deleting_the_file_is_how_an_identity_is_replaced_on_purpose() {
        // Knowingly, and never as a side effect of something going wrong.
        let scratch = Scratch::new("replaced");
        let first = load_or_make(&scratch.0).expect("a key");

        std::fs::remove_file(at(&scratch.0)).expect("removed");
        let second = load_or_make(&scratch.0).expect("a new key");

        assert_ne!(
            first.verifying_key().bytes(),
            second.verifying_key().bytes()
        );
    }
}
