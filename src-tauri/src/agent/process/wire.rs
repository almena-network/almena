//! What crosses the pipe once there is a child at the other end of it.
//!
//! Two threads read — one the child's records, one its events — and the calls in here write.
//! Nothing in this file decides whether the agent exists; that is [`super::lifecycle`], and the
//! seam matters because a change to what is said should not be a change to how one is started.
//!
//! # A frame nobody could read still ends the run it was about
//!
//! [`receive`] used to log a refusal and return, which left `running` set and no terminal event
//! on the channel: the screen span for ever and the composer stayed on *Stop*. That is the one
//! shape this file must never take again, and it is not hypothetical — a build meeting an agent
//! that names a capability it has never heard of gets exactly that frame, and `serde` refuses
//! the whole event rather than the field it could not place.

use std::io::{BufRead, BufReader, Write};
use std::time::{Duration, Instant};

use almena_agent_protocol::framing;
use almena_agent_protocol::message::{Command as Ask, CommandBody};
use almena_agent_protocol::vocabulary::ErrorCode;
use log::{info, warn};
use tauri::{AppHandle, Manager};

use crate::agent::exchange::{AgentEvent, Exchange, about, ready_of};
use crate::agent::process::{Refusal, Supervisor};
use crate::agent::records;

/// How long a cancel is given to be honoured before the process is ended instead.
const CANCEL_WITHIN: Duration = Duration::from_secs(2);

/// Reads the child's stderr for as long as it writes any, forwarding each line to the log.
pub(super) fn watch_records(stderr: std::process::ChildStderr) {
    std::thread::spawn(move || {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            records::forward(&line);
        }
    });
}

/// Reads the child's stdout for as long as it writes any, routing each event to its run.
pub(super) fn watch_events(app: AppHandle, stdout: std::process::ChildStdout) {
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
        super::lifecycle::gone(&app);
    });
}

/// Routes one frame the child wrote to whatever is waiting for it.
fn receive(app: &AppHandle, payload: &[u8]) {
    let event = match framing::decode_event(payload) {
        Ok(event) => event,
        Err(error) => {
            warn!("agent_event_not_understood reason={error}");
            // The stream is still in step — the length prefix was read and honoured, and only
            // what was inside could not be placed. So the child is left running and the run is
            // what ends, with the identifier the protocol itself chose for this failure.
            unanswerable(
                app,
                framing::identifier_of(payload).as_deref(),
                &error.code(),
            );
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

/// Ends the run in flight over a frame that could not be read, and tells the screen why.
///
/// `about_run` is whatever identifier the payload plainly carried, which is best-effort: a
/// frame that never held one yields `None` and is taken as being about the run in flight,
/// because the alternative is leaving that run unanswered for ever.
///
/// The failed-start latch is deliberately **not** released here. A run that ended in a frame
/// nobody could read is not evidence that this agent works, and treating it as evidence is how
/// a build that cannot be understood gets restarted for ever.
fn unanswerable(app: &AppHandle, about_run: Option<&str>, code: &ErrorCode) {
    let supervisor = app.state::<Supervisor>();
    let mut held = supervisor.held();

    let Some(running) = held.running.as_ref() else {
        return;
    };
    if about_run.is_some_and(|named| named != running.id) {
        return;
    }

    if running.tell(&AgentEvent::failed(code)) {
        held.running = None;
    }
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

    super::lifecycle::ensure_running(app)?;

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
    super::lifecycle::end(app);
    Ok(())
}
