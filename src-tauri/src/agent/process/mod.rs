//! The agent process: starting one, speaking to it, and ending it.
//!
//! # It is spawned by hand, and that is what keeps the permission surface at nothing
//!
//! Not through `tauri-plugin-shell`. Its sidecar mechanism copies **one file**, renamed for the
//! target triple, and the agent is a directory — the program is inert without the tree beside
//! it. Taking that route would mean a single-file build, which its own repository measured at
//! twelve seconds of startup for every run. And it would have cost a plugin, a
//! `shell:allow-execute` entry in the capabilities, and a path from the webview to *run a
//! program*. `std::process::Command` and two threads costs none of those, and no such path
//! exists anywhere in this application.
//!
//! # The pipe is the liveness signal
//!
//! Nothing here writes a PID file, makes a process group or opens anything under the runtime
//! directory. If this application is killed outright its end of the child's stdin closes with
//! it, the child reads the end of its input and stops on its own. An orphan is not possible,
//! and the reason is the transport rather than any bookkeeping.
//!
//! # One run at a time
//!
//! Refused in [`wire::ask`], before anything is written to the pipe, so the agent never meets a
//! case it has not promised to serve. The wire could carry two — every message is addressed —
//! and the day a runtime arrives that can serve two, this is the guard that moves and the wire
//! that does not.
//!
//! # Three files, and where the seam falls
//!
//! This module holds what everything else needs to agree on: the state, what a screen is told,
//! and the lock. [`lifecycle`] is the child's existence — spawning one, waiting for it to say
//! hello, and ending it. [`wire`] is what crosses the pipe once it exists. Neither half can be
//! described without the other's noun, and neither can be described with the word "and", which
//! is the test a module is held to here: say what it does without that word, or it is two.

mod lifecycle;
mod wire;

use std::process::{Child, ChildStdin};
use std::sync::{Mutex, MutexGuard};

use almena_agent_protocol::vocabulary::ErrorCode;
use log::{info, warn};
use serde::Serialize;
use tauri::AppHandle;

use crate::agent::bundled;
use crate::agent::exchange::{Exchange, Ready};

pub use lifecycle::end;
pub use wire::{ask, cancel};

/// Where the agent is, as far as this application knows.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum State {
    /// This build does not carry an agent at all.
    NotBundled,
    /// There is one, and nobody has asked for it yet.
    #[default]
    NotStarted,
    /// It was asked for, and it will not start.
    WillNotStart,
    /// It is running.
    Running,
    /// It was running, and it is not now.
    Stopped,
}

/// What a screen is told about the agent, without asking it anything.
///
/// Every figure is an `Option`, and that is not tidiness: a reading nobody took is `null` all
/// the way down, and a screen that drew a zero or an empty string for one would be claiming a
/// measurement nobody made.
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    /// Where it is.
    pub state: State,
    /// The model the **running** agent reported, or `null` while none is running.
    ///
    /// Not what was chosen. The two differ from the moment somebody changes the setting until
    /// the agent next starts, and a screen that showed one for the other would be answering a
    /// question nobody asked.
    pub model: Option<String>,
    /// What the running agent calls itself, or `null` while none is running.
    pub agent_version: Option<String>,
    /// The identifier of the run in flight, or `null` when none is.
    ///
    /// An identifier rather than the boolean this used to be, and the difference is a webview
    /// that reloaded. A page that has just mounted holds no memory of the run it started, so a
    /// boolean tells it only that it cannot ask anything — for as long as a run it can neither
    /// name nor cancel goes on. Naming the run is what lets the new page adopt it.
    pub in_flight: Option<String>,
}

/// Why an ask could not be accepted.
///
/// An identifier and never prose — what a person reads is looked up from it in the catalogs,
/// and only the interface holds those.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Refusal {
    /// The identifier.
    pub code: String,
}

impl Refusal {
    /// A refusal carrying one identifier.
    pub(crate) fn of(code: &ErrorCode) -> Self {
        Self {
            code: code.as_str().to_owned(),
        }
    }
}

/// Everything this application holds about the agent beside it.
#[derive(Default)]
pub struct Supervisor {
    held: Mutex<Held>,
}

/// The mutable half, behind the lock.
#[derive(Default)]
struct Held {
    state: State,
    child: Option<Child>,
    writing: Option<ChildStdin>,
    ready: Option<Ready>,
    running: Option<Exchange>,
    failed_starts: u8,
}

impl Supervisor {
    /// Nothing started, and nothing known yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// What a screen is told, without anything being asked of the agent.
    pub fn status(&self) -> Status {
        let held = self.held();
        Status {
            state: held.state,
            model: held.ready.as_ref().and_then(|ready| ready.model.clone()),
            agent_version: held.ready.as_ref().map(|ready| ready.agent_version.clone()),
            in_flight: held.running.as_ref().map(|running| running.id.clone()),
        }
    }

    /// Records at startup whether this build carries an agent at all.
    ///
    /// The one thing done before anybody asks for one, and it is done so that the screen can
    /// say *which* nothing it is showing rather than starting fifty megabytes of Python for
    /// everybody who never opens it.
    pub fn look_for_one(&self, app: &AppHandle) {
        let found = bundled::binary(app);
        let mut held = self.held();
        held.state = if found.is_some() {
            State::NotStarted
        } else {
            State::NotBundled
        };
        info!("agent_looked_for bundled={}", found.is_some());
    }

    /// The lock, taking a poisoned one back rather than ending the application over it.
    ///
    /// A reader thread panicking part-way through a frame poisons this, and the answer is to
    /// carry on with what is inside and stop the child, so that the next ask starts clean. A
    /// `unwrap` here would be an interface that vanishes mid-sentence, which the workspace
    /// lints deny for exactly this reason.
    fn held(&self) -> MutexGuard<'_, Held> {
        match self.held.lock() {
            Ok(held) => held,
            Err(poisoned) => {
                warn!("agent_state_poisoned");
                poisoned.into_inner()
            }
        }
    }
}
