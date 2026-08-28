//! Holding a node's directory, so that only one process is ever the node in it.
//!
//! **A node is a directory with a key in it, and two processes over one directory is a conflict
//! that gets refused.** Not out of tidiness: both would append to the same record and both would
//! close the same epochs, and what came out would be one identity with two histories interleaved —
//! which is, to anybody reading it, exactly what a node caught contradicting itself looks like.
//!
//! # The lock is the open file, not a note about one
//!
//! A file holding a process number is a promise that has to be kept by whoever wrote it, and a
//! machine that loses power keeps no promises: the note outlives the process and the next start
//! finds a directory it is told is busy and that nobody is in. Somebody then has to be told to
//! delete a file by hand, which is a design that has already failed.
//!
//! Here the **operating system** holds the lock, for exactly as long as the file is open. When the
//! process ends — cleanly, killed, or with the power cut — the handle closes and the lock is gone
//! with it. Nothing is ever left behind to clean up, and the file itself stays where it is, empty
//! and reusable, because deleting it would race with whoever is opening it next.

use std::fs::{File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};

/// What the lock is taken on.
///
/// Its contents are nobody's business and it has none. What matters is that it can be opened and
/// locked, which is a fact about the operating system rather than about anything written down.
const FILE: &str = "node.lock";

/// Why a directory could not be held.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotHeld {
    /// Somebody else is the node in this directory.
    ///
    /// The other process may be this same program, or a different way of running it: the window
    /// and the terminal are two faces over one node, and running both over one directory is the
    /// conflict rather than a clever way to get both.
    AlreadyHeld,
    /// The directory could not be made, or the file could not be opened.
    NotWritable,
    /// The operating system would not say whether it could be locked.
    ///
    /// Told apart from being held, because they call for different things: one is *close the other
    /// one*, and this is *this filesystem cannot do this*, which is what a network share often
    /// cannot. Coming up anyway would be deciding, on a filesystem that will not answer, that
    /// nobody else is there.
    CannotTell,
}

/// A directory, held for as long as this is kept.
///
/// **Dropping it lets go**, which is what makes the whole thing safe: there is no unlock anybody
/// can forget, and a process that dies in any manner at all releases it because the operating
/// system closes its files.
#[derive(Debug)]
#[must_use = "dropping this lets go of the directory, which is the opposite of taking it"]
pub struct Held {
    /// The open file. It is the lock; nothing is read from it or written to it.
    _file: File,
    /// Which directory is being held, so that it can be said.
    directory: PathBuf,
}

impl Held {
    /// The directory this is holding.
    #[must_use]
    pub fn directory(&self) -> &Path {
        &self.directory
    }
}

/// Where the lock of a node in `directory` lives.
#[must_use]
pub fn at(directory: &Path) -> PathBuf {
    directory.join(FILE)
}

/// Become the node in `directory`, or find out somebody already is.
///
/// # Errors
///
/// [`NotHeld`], and the three are worth telling apart because each is a different thing to do:
/// close the other node, fix the permissions, or move the directory off a filesystem that will not
/// lock.
pub fn hold(directory: &Path) -> Result<Held, NotHeld> {
    std::fs::create_dir_all(directory).map_err(|_| NotHeld::NotWritable)?;

    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(at(directory))
        .map_err(|_| NotHeld::NotWritable)?;

    match file.try_lock() {
        Ok(()) => Ok(Held {
            _file: file,
            directory: directory.to_owned(),
        }),
        Err(TryLockError::WouldBlock) => Err(NotHeld::AlreadyHeld),
        Err(TryLockError::Error(_)) => Err(NotHeld::CannotTell),
    }
}

#[cfg(test)]
mod tests {
    use super::{NotHeld, at, hold};
    use std::path::PathBuf;

    /// A directory of this test's own, removed when it is done with it.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!("almena-directory-{name}"));
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
    fn one_directory_is_held_by_one() {
        // The whole point. Both would append to the same record and close the same epochs, and one
        // identity with two histories interleaved is what a node caught contradicting itself looks
        // like to everybody else.
        let scratch = Scratch::new("one");
        let held = hold(&scratch.0).expect("nobody else is here");

        assert_eq!(hold(&scratch.0).err(), Some(NotHeld::AlreadyHeld));
        drop(held);
    }

    #[test]
    fn letting_go_lets_somebody_else_in() {
        // What makes it safe to be killed: the operating system closes the file, so nothing has to
        // be tidied up by whoever comes next.
        let scratch = Scratch::new("release");
        let held = hold(&scratch.0).expect("nobody else is here");
        drop(held);

        assert!(
            hold(&scratch.0).is_ok(),
            "and the next start is not told a directory is busy that nobody is in"
        );
    }

    #[test]
    fn the_file_is_left_where_it_is() {
        // Deleting it on the way out would race with whoever is opening it on the way in, and the
        // two would each hold a lock on a different file with the same name.
        let scratch = Scratch::new("stays");
        drop(hold(&scratch.0).expect("held"));
        assert!(at(&scratch.0).exists());
    }

    #[test]
    fn two_directories_are_two_nodes_and_neither_blocks_the_other() {
        let one = Scratch::new("two-one");
        let other = Scratch::new("two-other");

        let first = hold(&one.0).expect("held");
        let second = hold(&other.0).expect("held");
        assert_ne!(first.directory(), second.directory());
    }

    #[test]
    fn a_directory_that_does_not_exist_yet_gets_made() {
        let scratch = Scratch::new("fresh");
        let deeper = scratch.0.join("nested").join("further");
        assert!(hold(&deeper).is_ok());
        assert!(at(&deeper).exists());
    }
}
