//! What the operator chose, so that they are only asked once.
//!
//! Where a language comes from matters less than what happens when somebody disagrees with it:
//! what a person chooses overrules what the system says, and it is remembered. Detecting is a
//! courtesy at startup, not a decision imposed on anybody — and a program that detects but
//! cannot be told otherwise has only done the courtesy.
//!
//! The window and this are the same node with two faces, neither of them the cut-down version
//! of the other, and until now only one of them could be told what language to speak. That was
//! not a missing convenience; it was one face able to do something the other could not.
//!
//! # Its own file, in its own directory
//!
//! The same name and the same field as the windowed application's, and **not the same file**.
//! `almena-paths` says why: these are two applications — `network.almena.desktop` and
//! `network.almena.cli` — so separate directories, separate keys, and a machine running both is
//! two nodes. Two nodes do not share a configuration; they share a shape.
//!
//! # It is handed a directory rather than resolving one
//!
//! The resolver lives in `almena-paths` and the caller already holds it, so this module takes the
//! answer instead of asking again. That also makes every path here a test's to choose, which is
//! why nothing below has to reach for the environment of the machine running it.
//!
//! # Nothing here is personal
//!
//! A language is a choice about how a program behaves, not a fact about a person. That this file
//! holds one thing and that the one thing is that is worth keeping true.

use std::{fs, path::Path};

use log::warn;
use serde_json::Value;

/// The file the choices are kept in, inside the configuration directory.
const FILE: &str = "preferences.json";

/// The field the language is kept under — the windowed application's name for it.
const LANGUAGE: &str = "language";

/// Everything that was stored, or an empty object when nothing was.
///
/// Every failure lands on the same answer, and deliberately: a missing file is a first launch,
/// and a file this build cannot parse is one written by a version that meant something else by
/// it. Neither is a reason to refuse to start, and both leave the operator able to choose again.
fn read(directory: Option<&Path>) -> Value {
    let stored = directory
        .and_then(|directory| fs::read_to_string(directory.join(FILE)).ok())
        .and_then(|text| serde_json::from_str::<Value>(&text).ok());

    match stored {
        Some(value) if value.is_object() => value,
        _ => Value::Object(serde_json::Map::new()),
    }
}

/// The language that was chosen, if one was.
///
/// A free string rather than a [`crate::language::Language`], because what is stored may name a
/// language this build no longer ships — and deciding what to do about that is
/// [`crate::language::Language::settled`]'s job, not this file's.
#[must_use]
pub fn chosen(directory: Option<&Path>) -> Option<String> {
    read(directory)
        .get(LANGUAGE)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

/// Remembers a language, keeping whatever else the file held.
///
/// Read, change one field, write back — rather than writing a file with one key in it. A newer
/// build that stores something else beside the language must not lose it to an older one that
/// does not know about it: keeping something never required understanding it, and a file is no
/// different from an operation in that.
///
/// A failure to write is recorded and nothing else: the operator asked for a language and gets it
/// for this run either way. Making that fatal would be refusing to run over a preference.
pub fn remember(directory: Option<&Path>, language: &str) {
    let Some(directory) = directory else {
        warn!("preferences_not_written reason=no_configuration_directory");
        return;
    };

    let mut stored = read(Some(directory));
    if let Some(object) = stored.as_object_mut() {
        object.insert(LANGUAGE.to_owned(), Value::String(language.to_owned()));
    }

    if let Err(error) = fs::create_dir_all(directory) {
        warn!("preferences_not_written reason={error}");
        return;
    }

    match serde_json::to_string_pretty(&stored) {
        Ok(text) => {
            if let Err(error) = fs::write(directory.join(FILE), text + "\n") {
                warn!("preferences_not_written reason={error}");
            }
        }
        Err(error) => warn!("preferences_not_written reason={error}"),
    }
}

#[cfg(test)]
mod tests {
    use super::{FILE, LANGUAGE, chosen, remember};
    use serde_json::Value;
    use std::path::PathBuf;

    /// A directory of this test's own, removed when it is done with it.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!("almena-preferences-{name}"));
            let _ = std::fs::remove_dir_all(&path);
            Self(path)
        }

        fn path(&self) -> Option<&std::path::Path> {
            Some(self.0.as_path())
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn nothing_is_chosen_until_somebody_chooses() {
        let scratch = Scratch::new("nothing");
        assert_eq!(chosen(scratch.path()), None);
    }

    #[test]
    fn a_choice_survives_being_written_and_read_back() {
        let scratch = Scratch::new("round-trip");

        remember(scratch.path(), "es");
        assert_eq!(chosen(scratch.path()).as_deref(), Some("es"));

        remember(scratch.path(), "en");
        assert_eq!(chosen(scratch.path()).as_deref(), Some("en"));
    }

    #[test]
    fn what_another_build_stored_is_not_lost() {
        // An older build must not throw away what a newer one wrote just because it does not
        // know the field, exactly as a node keeps an operation it cannot read.
        let scratch = Scratch::new("preserve");
        std::fs::create_dir_all(&scratch.0).expect("the directory");
        std::fs::write(scratch.0.join(FILE), r#"{"theme":"dark","language":"en"}"#)
            .expect("the file");

        remember(scratch.path(), "es");

        let text = std::fs::read_to_string(scratch.0.join(FILE)).expect("the file back");
        let stored: Value = serde_json::from_str(&text).expect("json");
        assert_eq!(stored["theme"], "dark", "the other choice was dropped");
        assert_eq!(stored[LANGUAGE], "es");
    }

    #[test]
    fn a_file_this_build_cannot_read_is_a_first_launch() {
        let scratch = Scratch::new("unreadable");
        std::fs::create_dir_all(&scratch.0).expect("the directory");
        std::fs::write(scratch.0.join(FILE), "not json at all").expect("the file");

        assert_eq!(chosen(scratch.path()), None);

        // And it is still writable afterwards, rather than a state the program is stuck in.
        remember(scratch.path(), "es");
        assert_eq!(chosen(scratch.path()).as_deref(), Some("es"));
    }

    #[test]
    fn no_configuration_directory_is_not_a_crash() {
        // The platform would not say where home is. The program runs; it just forgets.
        assert_eq!(chosen(None), None);
        remember(None, "es");
        assert_eq!(chosen(None), None);
    }
}
