//! What a person chose: the interface's palette, its identity colour, its language, and which
//! model the agent is asked for.
//!
//! Four small facts, kept in one file so that they survive a restart, and kept **here** rather
//! than in the webview's own storage, which is somewhere nobody can find, back up or delete
//! with the conventions of their own operating system. They are what the user chose, so they
//! are configuration, so they live in the configuration directory, and the path comes from the
//! resolver rather than from a literal.
//!
//! **The vocabularies are not here.** This side knows that there are four choices and what
//! they are called; it does not know that a palette is `light` or `dark`, that a language is
//! `en` or `es`, or what any model is called. Those lists live where they are already written
//! down — `src/styles/tokens.css` for the two the interface is drawn from, `src/i18n/` for the
//! language, `src/lib/models.ts` for the model — and adding a fourth accent, a third language
//! or another model must not mean editing Rust as well. Anything this file does not recognise
//! is a string it stores and hands back, and the interface is what narrows it.
//!
//! Nothing in here is personal data and nothing ever will be: a palette, a colour, a language
//! and the name of a model are choices about how a program behaves, not facts about a person.

use std::{fs, path::PathBuf};

use log::{info, warn};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

/// The file the choices are kept in, inside the configuration directory.
const FILE: &str = "preferences.json";

/// What a person chose, with `None` wherever they have chosen nothing.
///
/// Absent is not the same as a default written down: it is the interface that decides what
/// "nobody has chosen a language" means, and its answer — the language the device asks for —
/// is not a value this side could have stored in advance.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Preferences {
    /// The palette, or `None` to follow the operating system.
    pub theme: Option<String>,
    /// The identity colour, or `None` for the one the application icon wears.
    pub accent: Option<String>,
    /// The language, or `None` for the one the device asks for.
    pub language: Option<String>,
    /// The model the agent is asked for, or `None` for the agent's own default.
    ///
    /// Absent means the variable is not set at all when the agent starts, so the agent decides.
    /// The alternative — a default written down here — would put the vocabulary back on this
    /// side through the door the paragraph above closes.
    pub model: Option<String>,
}

/// Where the file is, or nothing when the platform will not say.
///
/// A failure here is not worth ending anything over: an application that cannot find its
/// configuration directory runs with the defaults, which is what it does on a first launch.
fn file(app: &AppHandle) -> Option<PathBuf> {
    match app.path().app_config_dir() {
        Ok(directory) => Some(directory.join(FILE)),
        Err(error) => {
            warn!("preferences_not_located reason={error}");
            None
        }
    }
}

/// Reads what was stored, or the defaults when nothing was.
///
/// Every failure lands on the same answer, and deliberately: a missing file is a first launch,
/// and a file this build cannot parse is one written by a version that meant something else by
/// it. Neither is a reason to refuse to draw a screen, and both leave the person with an
/// interface they can set again.
fn read(app: &AppHandle) -> Preferences {
    let Some(path) = file(app) else {
        return Preferences::default();
    };

    let Ok(text) = fs::read_to_string(&path) else {
        return Preferences::default();
    };

    serde_json::from_str(&text).unwrap_or_else(|error| {
        warn!("preferences_not_understood reason={error}");
        Preferences::default()
    })
}

/// Writes the choices out, and says in the log when it could not.
///
/// # Errors
///
/// Returns whatever the filesystem raised: the directory not being creatable, or the file not
/// being writable.
fn write(app: &AppHandle, preferences: &Preferences) -> std::io::Result<()> {
    let Some(path) = file(app) else {
        return Ok(());
    };

    if let Some(directory) = path.parent() {
        fs::create_dir_all(directory)?;
    }

    let text = serde_json::to_string_pretty(preferences)
        .map_err(|error| std::io::Error::other(error.to_string()))?;

    fs::write(path, text)
}

/// What a person has chosen so far.
#[tauri::command]
pub fn preferences(app: AppHandle) -> Preferences {
    read(&app)
}

/// Stores the choices, and returns what is stored afterwards.
///
/// The answer is read back rather than echoed, for the reason `open_at_login` reads its own
/// setting back: a caller can then tell a change from a refusal by comparing the two, and a
/// control that slid across while nothing was written is the one failure worth avoiding.
///
/// The whole set is written at once rather than one field at a time, so that two choices made
/// in quick succession cannot race each other into a half-written file.
#[tauri::command]
pub fn set_preferences(app: AppHandle, preferences: Preferences) -> Preferences {
    if let Err(error) = write(&app, &preferences) {
        warn!("preferences_not_stored reason={error}");
    } else {
        info!("preferences_stored");
    }

    read(&app)
}
