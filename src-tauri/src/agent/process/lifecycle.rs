//! The child's existence: spawning one, waiting for it to say hello, and ending it.
//!
//! Everything here is about a process being there or not being there. What crosses the pipe
//! once one is there is [`super::wire`], and the two are apart because a change to how the
//! agent is started should not be a change to what it is told.

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use almena_agent_protocol::vocabulary::ErrorCode;
use log::{info, warn};
use tauri::{AppHandle, Manager};

use crate::agent::bundled;
use crate::agent::exchange::AgentEvent;
use crate::agent::process::{Refusal, State, Supervisor};

/// How long the agent has to announce itself before it is given up on.
///
/// Ten seconds against a measured cold start of about one and a half. Generous on purpose: the
/// machine this runs on may be doing something else, and the cost of waiting a moment longer is
/// nothing beside the cost of telling somebody their agent will not start when it would have.
const READY_WITHIN: Duration = Duration::from_secs(10);

/// How long the child is given to leave of its own accord once its input is closed.
const STOP_WITHIN: Duration = Duration::from_secs(1);

/// How many starts may fail in a row before the application stops trying.
///
/// A screen saying *it will not start* is worth more than a spinner over a process being
/// respawned forty times a minute. A start counts as failed when it could not be spawned, or
/// when it was spawned and said nothing within [`READY_WITHIN`] — which is also what catches a
/// child that starts and dies immediately, since it stops being `Running` before it is ready.
/// The latch is released by the first run that reaches a terminal event.
const STARTS_BEFORE_GIVING_UP: u8 = 3;

/// Starts the agent if it is not already running, and returns nothing when it will not.
///
/// # Errors
///
/// [`ErrorCode::AGENT_WILL_NOT_START`] when this build carries no agent, when three starts have
/// failed in a row, or when the one just started said nothing within [`READY_WITHIN`].
pub(super) fn ensure_running(app: &AppHandle) -> Result<(), Refusal> {
    let supervisor = app.state::<Supervisor>();
    {
        let held = supervisor.held();
        if held.state == State::Running {
            return Ok(());
        }
        if held.state == State::NotBundled || held.failed_starts >= STARTS_BEFORE_GIVING_UP {
            return Err(Refusal::of(&ErrorCode::AGENT_WILL_NOT_START));
        }
    }

    let Some(binary) = bundled::binary(app) else {
        supervisor.held().state = State::NotBundled;
        return Err(Refusal::of(&ErrorCode::AGENT_WILL_NOT_START));
    };

    match start(app, &binary) {
        Ok(()) => Ok(()),
        Err(()) => {
            let mut held = supervisor.held();
            held.failed_starts = held.failed_starts.saturating_add(1);
            held.state = State::WillNotStart;
            Err(Refusal::of(&ErrorCode::AGENT_WILL_NOT_START))
        }
    }
}

/// Spawns one agent and waits for it to announce itself.
///
/// # Errors
///
/// `Err(())` when the process could not be spawned, could not be spoken to, or said nothing
/// within [`READY_WITHIN`]. Every one of them is logged here, so the caller has only to decide
/// what to tell the screen.
fn start(app: &AppHandle, binary: &PathBuf) -> Result<(), ()> {
    let scratch = crate::agent::scratch(app);
    let mut command = Command::new(binary);
    command
        .current_dir(&scratch)
        // Cleared, then filled in. The agent's own promise is that no credential can be handed
        // to it, and the way that promise dies quietly is an `OPENAI_API_KEY` sitting in the
        // shell that launched this application and being inherited two processes down. This
        // turns a property the agent asserts into one this side imposes.
        .env_clear()
        .envs(minimum_environment())
        .env("ALMENA_AGENT_RESOURCES", &scratch)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    if let Some(model) = crate::preferences::preferences(app.clone()).model {
        // Set only when somebody has chosen one. Where nobody has, the agent's own default
        // applies — this side deliberately does not know what a model is called, for the same
        // reason `preferences` does not know what a palette is called.
        command.env("ALMENA_AGENT_MODEL", model);
    }

    without_a_console(&mut command);

    let mut child = command.spawn().map_err(|error| {
        warn!("agent_would_not_spawn reason={error}");
    })?;

    let Some(stdout) = child.stdout.take() else {
        warn!("agent_has_no_stdout");
        let _ = child.kill();
        return Err(());
    };
    let Some(stderr) = child.stderr.take() else {
        warn!("agent_has_no_stderr");
        let _ = child.kill();
        return Err(());
    };
    let writing = child.stdin.take();

    {
        let supervisor = app.state::<Supervisor>();
        let mut held = supervisor.held();
        held.child = Some(child);
        held.writing = writing;
        held.ready = None;
        held.state = State::Running;
    }

    super::wire::watch_records(stderr);
    super::wire::watch_events(app.clone(), stdout);

    wait_for_ready(app)
}

/// The variables the agent is started with, and nothing else.
///
/// A short list on purpose: everything the agent is configured by arrives as an
/// `ALMENA_AGENT_*` variable set above, and what remains here is only what an operating system
/// needs in order to run a process at all.
fn minimum_environment() -> Vec<(String, String)> {
    let wanted: &[&str] = if cfg!(windows) {
        &["SYSTEMROOT", "TEMP", "PATHEXT"]
    } else {
        &["TMPDIR"]
    };

    wanted
        .iter()
        .filter_map(|name| {
            std::env::var(name)
                .ok()
                .map(|held| ((*name).to_owned(), held))
        })
        .collect()
}

/// Keeps the child from opening a console window of its own on Windows.
///
/// The agent is built as a console program, because its whole interface is three pipes. A
/// console program started from a windowed one gets a console — on screen, on every start —
/// unless it is asked not to. Building it windowed instead is not the fix: that detaches the
/// standard streams and takes the protocol with them.
#[cfg(windows)]
fn without_a_console(command: &mut Command) {
    use std::os::windows::process::CommandExt;

    /// `CREATE_NO_WINDOW`, from the Windows process creation flags.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    command.creation_flags(CREATE_NO_WINDOW);
}

/// Nothing to do: no other desktop opens a window for a child process.
#[cfg(not(windows))]
fn without_a_console(_command: &mut Command) {}

/// Waits until the child has announced itself, or gives up on it.
///
/// # Errors
///
/// `Err(())` when nothing arrived within [`READY_WITHIN`], having ended the child first: a
/// process that will not say hello is not one to leave running.
fn wait_for_ready(app: &AppHandle) -> Result<(), ()> {
    let supervisor = app.state::<Supervisor>();
    let since = Instant::now();

    while since.elapsed() < READY_WITHIN {
        {
            let held = supervisor.held();
            if held.ready.is_some() {
                return Ok(());
            }
            if held.state != State::Running {
                warn!("agent_stopped_before_it_said_anything");
                return Err(());
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }

    warn!(
        "agent_said_nothing_within seconds={}",
        READY_WITHIN.as_secs()
    );
    end(app);
    Err(())
}

/// Notes that the child has gone, and fails whatever was in flight.
pub(super) fn gone(app: &AppHandle) {
    let supervisor = app.state::<Supervisor>();
    let mut held = supervisor.held();

    held.state = State::Stopped;
    held.writing = None;
    held.ready = None;
    if let Some(child) = held.child.as_mut() {
        info!("agent_exited status={:?}", child.try_wait().ok().flatten());
    }
    held.child = None;

    if let Some(running) = held.running.take() {
        running.tell(&AgentEvent::failed(&ErrorCode::AGENT_STOPPED));
    }
}

/// Ends the agent, if one is running.
///
/// Its input is closed first, which is how the agent is asked: it reads the end of its stdin,
/// finishes anything it can, and stops. Only a child that has not gone within [`STOP_WITHIN`]
/// is killed.
pub fn end(app: &AppHandle) {
    let supervisor = app.state::<Supervisor>();

    // Dropped here rather than inside the wait, so the child sees the end of its input while
    // this thread is not holding the lock its reader will want.
    let mut child = {
        let mut held = supervisor.held();
        held.writing = None;
        held.child.take()
    };

    let Some(child) = child.as_mut() else {
        return;
    };

    let since = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                info!("agent_stopped status={status}");
                return;
            }
            Ok(None) if since.elapsed() < STOP_WITHIN => {
                std::thread::sleep(Duration::from_millis(20));
            }
            _ => break,
        }
    }

    warn!("agent_would_not_stop killed=true");
    let _ = child.kill();
    let _ = child.wait();
}
