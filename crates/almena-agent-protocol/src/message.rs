//! Every message either side sends, and the version each one carries.
//!
//! # The shape on the wire
//!
//! One message is one MessagePack map with string keys. Two of those keys are always there:
//! `contract_version`, and one of `command` or `event` naming what the rest of the map holds.
//! A frame therefore says **which direction it is** before it says anything else, which is what
//! lets a reader refuse a reply arriving where a request belongs without decoding either.
//!
//! ```text
//! { "contract_version": "2", "event": "token", "id": "7", "text": "hola" }
//! ```
//!
//! # The version is on every frame
//!
//! Not on a handshake, on **every** frame, and the reason is worth keeping: a reader that
//! checks once has to trust that nothing changed under it, and a reader that checks each time
//! does not. [`super::framing::decode_command`] and [`super::framing::decode_event`] check it
//! before they look at anything else, so a frame from a version this build does not speak is
//! refused as exactly that rather than as whatever field happened to be missing.
//!
//! # One run at a time is not written down here
//!
//! Every message carries an `id`, so the wire could carry two runs at once and tell their
//! events apart. That the application refuses to start a second run while one is in flight is
//! a decision of the side doing the refusing, not a property of this contract — which is what
//! lets a runtime that can serve two of them arrive later without the wire changing.

use serde::{Deserialize, Serialize};

use crate::vocabulary::{ErrorCode, Intent, Stage, Suggestion, ToolName, Turn};

/// The version of this contract, on every frame in both directions.
///
/// Bumped whenever a field changes meaning, is added or goes. A build refuses a frame naming
/// anything else rather than guessing which half of it it still understands.
pub const CONTRACT_VERSION: &str = "2";

/// One message from the application to the agent.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Command {
    /// The contract this frame is written in. Always [`CONTRACT_VERSION`] when this side wrote
    /// it.
    pub contract_version: String,
    /// Which message it is, and what it carries.
    #[serde(flatten)]
    pub body: CommandBody,
}

impl Command {
    /// A command of this build's contract version.
    #[must_use]
    pub fn new(body: CommandBody) -> Self {
        Self {
            contract_version: CONTRACT_VERSION.to_owned(),
            body,
        }
    }
}

/// What a command is asking for.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum CommandBody {
    /// Answer this. The only command that produces events.
    Run {
        /// The application's identifier for this run. Every event about it carries the same one.
        id: String,
        /// What is being asked for.
        intent: Intent,
        /// What the run is given to work with.
        params: Params,
    },
    /// Stop the run named, as soon as it can be stopped.
    ///
    /// Idempotent, and a cancel for a run that is not in flight is answered with **nothing**: a
    /// cancel racing a completion is ordinary, and replying would put a second terminal event
    /// on a run that already ended.
    Cancel {
        /// The run to stop.
        id: String,
    },
    /// What came of a [`EventBody::ToolCall`].
    ToolResult {
        /// The run that asked.
        id: String,
        /// The call being answered.
        call_id: String,
        /// What the capability produced, or `None` where the application declined to run it.
        /// Declining is an answer, not a failure.
        output: Option<String>,
    },
}

/// What a run is given to work with.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct Params {
    /// The conversation so far, oldest first.
    #[serde(default)]
    pub messages: Vec<Turn>,
    /// The names of what the agent may read. It never chooses these itself.
    #[serde(default)]
    pub resources: Vec<String>,
    /// The capabilities the application is willing to perform for this run.
    ///
    /// Empty means the run may ask for nothing, which is every run today. A `tool_call` naming
    /// anything absent from this list is refused before it reaches the application.
    #[serde(default)]
    pub tools: Vec<ToolName>,
}

/// One message from the agent to the application.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Event {
    /// The contract this frame is written in.
    pub contract_version: String,
    /// Which message it is, and what it carries.
    #[serde(flatten)]
    pub body: EventBody,
}

/// What an event is reporting.
///
/// # The one invariant
///
/// Every [`CommandBody::Run`] produces exactly one [`EventBody::Started`], then zero or more
/// events carrying content, then exactly one of [`EventBody::Completed`],
/// [`EventBody::Failed`] or [`EventBody::Cancelled`]. **No event for that run follows the
/// terminal one.** That is what lets a reader release everything it was holding for a run the
/// moment a terminal arrives, with no timer and no bookkeeping.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum EventBody {
    /// The agent is up, before anything has been asked of it.
    ///
    /// Sent once, unprompted. It is both the version check and the only honest source of what
    /// the agent is *actually* running — which is a different fact from what was chosen, and
    /// the two are worth showing separately.
    Ready {
        /// What the agent calls itself.
        agent_version: String,
        /// The model it was started with, or `None` where it was told nothing and fell back to
        /// its own default.
        model: Option<String>,
    },
    /// The run was admitted. Always the first event about it.
    ///
    /// It exists so that *nobody has looked yet* and *it is working and has produced nothing*
    /// stay two facts. Without it they are one, and a screen has to invent which.
    Started {
        /// The run.
        id: String,
    },
    /// A stage of answering began, or moved.
    Progress {
        /// The run.
        id: String,
        /// Which part of answering it has reached.
        stage: Stage,
        /// How much of the stage is done, or `None` where nothing counted it. Never `0` for
        /// *unknown*: a count of nought is a measurement, and this field's absence is the lack
        /// of one.
        done: Option<u32>,
        /// How much there is to do, or `None` where nothing counted it.
        total: Option<u32>,
    },
    /// One piece of a streamed answer. [`Intent::Chat`] only.
    Token {
        /// The run.
        id: String,
        /// The piece, in the order it was produced.
        text: String,
    },
    /// The run is asking the application to perform a capability on its behalf.
    ///
    /// The application executes; the agent never does. A call naming a capability absent from
    /// the run's [`Params::tools`] never reaches here.
    ToolCall {
        /// The run.
        id: String,
        /// This call, so that its result can be matched back to it. Scoped to the run.
        call_id: String,
        /// Which capability.
        name: ToolName,
        /// What to perform it with. Flat and string-valued: without nesting there is no
        /// structure for anything to hide inside.
        arguments: std::collections::BTreeMap<String, String>,
    },
    /// The single answer to a [`Intent::Propose`] run.
    Proposal {
        /// The run.
        id: String,
        /// What is being suggested. Prose, and inert.
        suggestion: Suggestion,
    },
    /// The run finished, having done what was asked. Terminal.
    Completed {
        /// The run.
        id: String,
    },
    /// The run stopped because a [`CommandBody::Cancel`] arrived. Terminal.
    ///
    /// **Not a retraction.** Tokens already sent were sent, and stay sent; this says only that
    /// no further event for the run is coming.
    Cancelled {
        /// The run.
        id: String,
    },
    /// The run was not answered, and this is why. Terminal.
    Failed {
        /// The run, or `None` for a frame that could not be attributed to one at all.
        id: Option<String>,
        /// Why, as an identifier the reader translates.
        code: ErrorCode,
        /// English, for a log. **Never drawn on a screen** — it is prose from a subprocess, and
        /// what a person reads is decided by whoever holds the catalogs.
        detail: String,
    },
}
