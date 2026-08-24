//! The agent this application runs beside itself.
//!
//! `almena-agent` is a program of its own, in a repository of its own, in a language of its
//! own. It is bundled inside this application and run as a child process, and everything this
//! side knows about it is the Agent Protocol — `almena-agent-protocol`, which names no graph,
//! no model and no framework. That is what lets the program at the other end be replaced
//! without a word here changing.
//!
//! **The windowed application alone**, of the two programs this repository builds. The other
//! is the CLI, which brings a node up on a machine in a rack, and an agent is something a
//! person sits in front of — `.agents/rules/deployments.md`.
//!
//! One file per concern, because "locating, starting, framing, routing, ending and forwarding
//! records" is six modules sharing a name.

use std::path::PathBuf;

use tauri::{AppHandle, Manager};

pub mod bundled;
pub mod commands;
pub mod exchange;
pub mod process;
pub mod records;

/// The directory the agent is given to work in, and to read resources from.
///
/// Empty, and it stays empty: this application hands the agent no text and names no resource,
/// so nothing is written here and nothing is read out of it. It exists because the agent's own
/// default for that setting is a **relative** path, which from an installed application
/// resolves somewhere absurd — so it is given somewhere real and harmless instead.
///
/// Under the cache directory, which is the right purpose by
/// `.agents/rules/storage-and-logs.md`: deleting it while the application is closed costs
/// nothing at all, which is the test that rule sets for cache.
pub fn scratch(app: &AppHandle) -> PathBuf {
    let directory = app
        .path()
        .app_cache_dir()
        .unwrap_or_else(|_| std::env::temp_dir())
        .join("agent");

    // A failure here is not worth ending anything over: the agent is started with it either
    // way, and a directory that is not there is one it reads nothing out of, which is already
    // the arrangement.
    if let Err(error) = std::fs::create_dir_all(&directory) {
        log::warn!("agent_scratch_not_created reason={error}");
    }

    directory
}
