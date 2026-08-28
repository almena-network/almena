//! Where the agent that ships inside this application is, on each platform.
//!
//! One question, asked once at startup and again whenever one is started, so that the screen
//! can tell *there is no agent in this build* from *there is one and nobody has asked for it*
//! — two facts about an empty screen, and one sentence for both is wrong half the time.

use std::path::PathBuf;

use tauri::{AppHandle, Manager, path::BaseDirectory};

/// The directory the agent's own files are bundled under, inside the application's resources.
const DIRECTORY: &str = "almena-agent";

/// The name of the program inside it.
const PROGRAM: &str = "almena-agent";

/// Where the bundled agent is, or nothing when this build does not carry one.
///
/// A build with no agent staged into it is an ordinary state rather than a failure: a
/// contributor who cloned this repository alone has no agent to stage, and the application
/// still builds, runs, and says on its own screen that it has none.
///
/// The suffix comes from [`std::env::consts::EXE_SUFFIX`] rather than from a `cfg`, for the
/// reason no path in this application is written by hand: a location decided in
/// platform-specific code is one that drifts per platform, and this one does not have to.
#[must_use]
pub fn binary(app: &AppHandle) -> Option<PathBuf> {
    let within =
        PathBuf::from(DIRECTORY).join(format!("{PROGRAM}{}", std::env::consts::EXE_SUFFIX));

    let found = app
        .path()
        .resolve(&within, BaseDirectory::Resource)
        .ok()
        .filter(|path| path.is_file())?;

    Some(found)
}
