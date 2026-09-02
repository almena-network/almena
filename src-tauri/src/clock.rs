//! Epochs added to the wall clock's, read from a file — a knob for the development network.
//!
//! **A network opened this morning has to be walkable this morning.** A device the words add
//! waits three days before it may sign, and everything a holder does after that first act is
//! signed by a device — so on a network whose clock is the wall's, nothing past the first step
//! can be tried until three days have passed. This is how the days pass instead: a file holding
//! one integer, the epochs to add, **re-read on every look at the clock** so that a test moves it
//! and every node reading the same file moves together.
//!
//! It reaches the development network alone. The window reads the file's path from
//! `ALMENA_CLOCK_OFFSET_FILE` while developing — the same reading the terminal's
//! `--clock-offset-file` does, and nothing a deployment sets — and a node on production ignores
//! it and says so once, because a node whose clock somebody can move is a node that signs roots
//! for hours that have not happened.
//!
//! The reading is deliberately forgiving in one direction only: a file that is absent, cannot
//! be read or does not hold an integer counts as nought, said once in the records as
//! `clock_offset_unreadable`, and the clock is the wall's. Every change in what the file says is
//! `clock_offset epochs=N`, so whoever reads the records can see the days pass.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};

use log::{info, warn};

/// What the file said before anybody read it, so that the first reading is said out loud too.
const NEVER_READ: i64 = i64::MIN;

/// Epochs added to the wall clock's, or nothing to leave the clock alone.
#[derive(Debug)]
pub struct Offset {
    /// The file to read on every look, or nothing when no knob was asked for.
    file: Option<PathBuf>,
    /// What the file said last time, so a change is said once and not on every look.
    last: AtomicI64,
    /// Whether the file being unreadable has already been said.
    complained: AtomicBool,
}

impl Default for Offset {
    fn default() -> Self {
        Self::none()
    }
}

impl Offset {
    /// No knob: the clock is the wall's and nothing is read.
    #[must_use]
    pub const fn none() -> Self {
        Self {
            file: None,
            last: AtomicI64::new(NEVER_READ),
            complained: AtomicBool::new(false),
        }
    }

    /// Read the epochs to add from this file, on every look.
    #[must_use]
    pub const fn reading(file: PathBuf) -> Self {
        Self {
            file: Some(file),
            last: AtomicI64::new(NEVER_READ),
            complained: AtomicBool::new(false),
        }
    }

    /// The wall clock's epoch number with the file's epochs added — or as it is, with no file.
    ///
    /// Never past the ends: an offset that would carry the epoch below nought or over the top
    /// stops at the end, because an epoch is a position on a clock and not a sum.
    #[must_use]
    pub fn applied(&self, wall: u64) -> u64 {
        let Some(file) = &self.file else {
            return wall;
        };
        let epochs = match std::fs::read_to_string(file) {
            Ok(text) => match text.trim().parse::<i64>() {
                Ok(epochs) => epochs,
                Err(_) => self.unreadable(file, "not_an_integer"),
            },
            Err(why) if why.kind() == std::io::ErrorKind::NotFound => {
                self.unreadable(file, "absent")
            }
            Err(_) => self.unreadable(file, "unreadable"),
        };
        // Said once per change, whichever thread looked first: the previous reading is swapped
        // out atomically, so two looks at once cannot both find it changed.
        if self.last.swap(epochs, Ordering::Relaxed) != epochs {
            info!("clock_offset epochs={epochs}");
        }
        wall.saturating_add_signed(epochs)
    }

    /// Nought, and the reason said once.
    fn unreadable(&self, file: &std::path::Path, reason: &str) -> i64 {
        if !self.complained.swap(true, Ordering::Relaxed) {
            warn!(
                "clock_offset_unreadable path={} reason={reason}",
                file.display()
            );
        }
        0
    }
}

#[cfg(test)]
mod tests {
    use super::Offset;

    /// A file of this test's own, removed when it is done with it.
    struct Scratch(std::path::PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!("almena-app-clock-{name}"));
            let _ = std::fs::remove_file(&path);
            Self(path)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    #[test]
    fn with_no_file_the_clock_is_the_wall_s() {
        let offset = Offset::none();
        assert_eq!(offset.applied(0), 0);
        assert_eq!(offset.applied(41), 41);
    }

    #[test]
    fn the_file_s_epochs_are_added_on_every_look() {
        // **Re-read every time**, because the file is what a test moves while the node runs.
        let scratch = Scratch::new("moves");
        std::fs::write(&scratch.0, "5\n").expect("written");
        let offset = Offset::reading(scratch.0.clone());
        assert_eq!(offset.applied(10), 15);

        std::fs::write(&scratch.0, "  72 ").expect("written");
        assert_eq!(
            offset.applied(10),
            82,
            "whitespace around the number is nothing"
        );

        std::fs::write(&scratch.0, "-3").expect("written");
        assert_eq!(offset.applied(10), 7, "and a step back is a step back");
        assert_eq!(offset.applied(1), 0, "never below nought");
    }

    #[test]
    fn a_file_that_cannot_be_read_counts_as_nought() {
        // Forgiving in one direction only: the clock is the wall's, and the records say why.
        let scratch = Scratch::new("absent");
        let offset = Offset::reading(scratch.0.clone());
        assert_eq!(offset.applied(10), 10, "absent");

        std::fs::write(&scratch.0, "three").expect("written");
        assert_eq!(offset.applied(10), 10, "not an integer");

        std::fs::write(&scratch.0, "4").expect("written");
        assert_eq!(offset.applied(10), 14, "and readable again is read again");
    }
}
