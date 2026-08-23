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
//! Refused **here**, before anything is written to the pipe, so the agent never meets a case it
//! has not promised to serve. The wire could carry two — every message is addressed — and the
//! day a runtime arrives that can serve two, this is the guard that moves and the wire that
//! does not.

use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

use almena_agent_protocol::framing;
use almena_agent_protocol::message::{Command as Ask, CommandBody};
use almena_agent_protocol::vocabulary::ErrorCode;
use log::{info, warn};
use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::agent::bundled;
use crate::agent::exchange::{AgentEvent, Exchange, Ready, about, ready_of};
use crate::agent::records;

/// How long the agent has to announce itself before it is given up on.
///
/// Ten seconds against a measured cold start of about one and a half. Generous on purpose: the
/// machine this runs on may be doing something else, and the cost of waiting a moment longer is
/// nothing beside the cost of telling somebody their agent will not start when it would have.
const READY_WITHIN: Duration = Duration::from_secs(10);

/// How long a cancel is given to be honoured before the process is ended instead.
const CANCEL_WITHIN: Duration = Duration::from_secs(2);

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
/// measurement — `.agents/rules/honest-emptiness.md`.
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
    /// Whether the run in flight, if there is one, is this application's.
    pub busy: bool,
}

/// Why an ask could not be accepted.
///
/// An identifier and never prose — `.agents/rules/user-facing-text.md`. What a person reads is
/// looked up from it in the catalogs.
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
            busy: held.running.is_some(),
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

/// Starts the agent if it is not already running, and returns nothing when it will not.
///
/// # Errors
///
/// [`ErrorCode::AGENT_WILL_NOT_START`] when this build carries no agent, when three starts have
/// failed in a row, or when the one just started said nothing within [`READY_WITHIN`].
fn ensure_running(app: &AppHandle) -> Result<(), Refusal> {
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

    watch_records(stderr);
    watch_events(app.clone(), stdout);

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

/// Reads the child's stderr for as long as it writes any, forwarding each line to the log.
fn watch_records(stderr: std::process::ChildStderr) {
    std::thread::spawn(move || {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            records::forward(&line);
        }
    });
}

/// Reads the child's stdout for as long as it writes any, routing each event to its run.
fn watch_events(app: AppHandle, stdout: std::process::ChildStdout) {
    std::thread::spawn(move || {
        let mut reading = BufReader::new(stdout);
        loop {
            match framing::read(&mut reading) {
                Ok(Some(payload)) => receive(&app, &payload),
                // The end of the input, cleanly: the child has gone.
                Ok(None) => break,
                Err(error) => {
                    warn!("agent_frame_refused reason={error}");
                    break;
                }
            }
        }
        gone(&app);
    });
}

/// Routes one frame the child wrote to whatever is waiting for it.
fn receive(app: &AppHandle, payload: &[u8]) {
    let event = match framing::decode_event(payload) {
        Ok(event) => event,
        Err(error) => {
            warn!("agent_event_not_understood reason={error}");
            return;
        }
    };

    let supervisor = app.state::<Supervisor>();

    if let Some(ready) = ready_of(&event) {
        info!(
            "agent_ready version={} model={:?}",
            ready.agent_version, ready.model
        );
        supervisor.held().ready = Some(ready);
        return;
    }

    let Some((about_run, shown)) = about(event) else {
        return;
    };

    let mut held = supervisor.held();
    let Some(running) = held.running.as_ref() else {
        return;
    };
    // An event about a run this side is not waiting for, which can only be a run it gave up
    // on. Dropped rather than delivered to whatever is listening now.
    if about_run.is_some_and(|named| named != running.id) {
        return;
    }

    if running.tell(&shown) {
        held.running = None;
        held.failed_starts = 0;
    }
}

/// Notes that the child has gone, and fails whatever was in flight.
fn gone(app: &AppHandle) {
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

/// Writes one command to the child.
///
/// # Errors
///
/// [`ErrorCode::AGENT_STOPPED`] when there is nothing to write to, or the pipe refused it.
fn write(app: &AppHandle, ask: &Ask) -> Result<(), Refusal> {
    let supervisor = app.state::<Supervisor>();
    let mut held = supervisor.held();

    let Some(writing) = held.writing.as_mut() else {
        return Err(Refusal::of(&ErrorCode::AGENT_STOPPED));
    };

    let frame = framing::encode(ask).map_err(|error| {
        warn!("agent_command_not_encoded reason={error}");
        Refusal::of(&ErrorCode::AGENT_STOPPED)
    })?;

    writing
        .write_all(&frame)
        .and_then(|()| writing.flush())
        .map_err(|error| {
            warn!("agent_command_not_written reason={error}");
            Refusal::of(&ErrorCode::AGENT_STOPPED)
        })
}

/// Starts a run, sending everything it produces to `channel`.
///
/// # Errors
///
/// [`ErrorCode::RUN_ALREADY_IN_FLIGHT`] when one is already running,
/// [`ErrorCode::AGENT_WILL_NOT_START`] when there is no agent to ask, and
/// [`ErrorCode::AGENT_STOPPED`] when the one there was has gone.
pub fn ask(app: &AppHandle, ask: Ask, exchange: Exchange) -> Result<(), Refusal> {
    {
        let supervisor = app.state::<Supervisor>();
        if supervisor.held().running.is_some() {
            return Err(Refusal::of(&ErrorCode::RUN_ALREADY_IN_FLIGHT));
        }
    }

    ensure_running(app)?;

    {
        let supervisor = app.state::<Supervisor>();
        supervisor.held().running = Some(exchange);
    }

    write(app, &ask).inspect_err(|_| {
        let supervisor = app.state::<Supervisor>();
        supervisor.held().running = None;
    })
}

/// Asks for the run in flight to stop, and ends the agent if it will not.
///
/// # Errors
///
/// [`ErrorCode::RUN_UNKNOWN`] when that run is not the one in flight.
pub fn cancel(app: &AppHandle, id: &str) -> Result<(), Refusal> {
    let supervisor = app.state::<Supervisor>();
    {
        let held = supervisor.held();
        match held.running.as_ref() {
            Some(running) if running.id == id => {}
            _ => return Err(Refusal::of(&ErrorCode::RUN_UNKNOWN)),
        }
    }

    write(app, &Ask::new(CommandBody::Cancel { id: id.to_owned() }))?;

    let since = Instant::now();
    while since.elapsed() < CANCEL_WITHIN {
        if supervisor.held().running.is_none() {
            info!("agent_cancel_confirmed");
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(20));
    }

    // It would not stop. The agent holds nothing between runs, so ending it costs the run
    // that was already being abandoned and nothing else — and the next ask starts a fresh one.
    warn!("agent_cancel_forced");
    end(app);
    Ok(())
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
