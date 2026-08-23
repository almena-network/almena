//! What the interface may ask of the agent.
//!
//! Four calls, and they are the whole of the boundary. A screen never sees a frame, a contract
//! version or a process: it asks a question, and events arrive on a channel until one of them
//! ends the run.

use almena_agent_protocol::message::{Command as Ask, CommandBody, Params};
use almena_agent_protocol::vocabulary::{ErrorCode, Intent, Turn};
use serde::Deserialize;
use tauri::AppHandle;
use tauri::ipc::Channel;

use crate::agent::exchange::{AgentEvent, Exchange};
use crate::agent::process::{self, Refusal, Status, Supervisor};

/// One thing to ask the agent.
///
/// A struct rather than four arguments, and the reason is a limit rather than taste: `AppHandle`
/// counts as a parameter, so the loose form would be six against a threshold of five —
/// `.agents/rules/code-size.md`. It is the better shape anyway.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Question {
    /// The interface's identifier for this run. Every event about it is sent on the channel
    /// that came with it, so nothing has to be correlated on the other side.
    pub id: String,
    /// What is being asked for.
    pub intent: Intent,
    /// The conversation so far, oldest first.
    pub messages: Vec<Turn>,
}

/// Where the agent is, without anything being asked of it.
#[tauri::command]
pub fn agent_status(app: AppHandle) -> Status {
    tauri::Manager::state::<Supervisor>(&app).status()
}

/// Asks the agent something, and sends everything it produces to `on_event`.
///
/// Returns as soon as the question has been handed over. **Every result arrives on the
/// channel**, failures included; what this answers is the one question the caller cannot get
/// from there, which is whether the question was accepted at all.
///
/// # Errors
///
/// [`Refusal`] carrying `run_already_in_flight`, `agent_will_not_start` or `agent_stopped`.
#[tauri::command]
pub async fn agent_ask(
    app: AppHandle,
    question: Question,
    on_event: Channel<AgentEvent>,
) -> Result<(), Refusal> {
    let exchange = Exchange {
        id: question.id.clone(),
        channel: on_event,
    };

    let ask = Ask::new(CommandBody::Run {
        id: question.id,
        intent: question.intent,
        params: Params {
            messages: question.messages,
            // The application hands over nothing to read, and offers no capability. Both are
            // empty rather than absent, and both are the application's to decide — the agent
            // never chooses either for itself.
            resources: Vec::new(),
            tools: Vec::new(),
        },
    });

    // A cold start is over a second of Python, and the main thread is drawing a window.
    let started = tauri::async_runtime::spawn_blocking(move || process::ask(&app, ask, exchange));

    joined(started.await, "agent_ask_not_run")
}

/// Asks for the run in flight to stop.
///
/// The run ends either way: what a person asked for was for it to stop, and it stops. Whether
/// the agent honoured the request or had to be ended is a difference the log records and the
/// screen does not — telling somebody *it would not stop, so it was killed* would be reporting
/// this application's business as though it were theirs.
///
/// # Errors
///
/// [`Refusal`] carrying `run_unknown` when that run is not the one in flight.
#[tauri::command]
pub async fn agent_cancel(app: AppHandle, id: String) -> Result<(), Refusal> {
    let stopped = tauri::async_runtime::spawn_blocking(move || process::cancel(&app, &id));

    joined(stopped.await, "agent_cancel_not_run")
}

/// What came back from work done off the main thread.
///
/// A thread that did not finish is the same answer to a caller as an agent that has gone: what
/// they asked for did not happen, and there is nothing on the channel coming. Which of the two
/// it was is a fact about this application, so it goes to the log and not to the screen.
fn joined<E: std::fmt::Display>(
    outcome: Result<Result<(), Refusal>, E>,
    what: &str,
) -> Result<(), Refusal> {
    outcome.unwrap_or_else(|error| {
        log::warn!("{what} reason={error}");
        Err(Refusal::of(&ErrorCode::AGENT_STOPPED))
    })
}

/// Ends the agent, so that the next question starts a fresh one.
///
/// What the Settings screen offers after the model is changed: the model a run uses is fixed
/// when the agent starts, so *this applies when it next starts* is a sentence with a control
/// behind it rather than an instruction to close the application.
#[tauri::command]
pub async fn agent_stop(app: AppHandle) {
    let _ = tauri::async_runtime::spawn_blocking(move || process::end(&app)).await;
}
