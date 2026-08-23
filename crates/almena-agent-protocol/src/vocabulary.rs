//! The closed vocabularies of the protocol, and the small values built out of them.
//!
//! Apart from [`super::message`] because these are the words, and that module is the sentences.
//! A word here is a decision about what may be said at all — which is why most of them are
//! enums that refuse anything not listed, and why the one that is not says so in its own
//! documentation.

use std::borrow::Cow;

use serde::{Deserialize, Serialize};

/// What a run is being asked for.
///
/// Two, and a closed set on purpose: a runtime that is not this one implements *these*, rather
/// than an open list it discovers by reading somebody's graph.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Intent {
    /// Prose, streamed a piece at a time.
    Chat,
    /// One suggestion, delivered whole.
    Propose,
}

/// Which part of answering a run has reached.
///
/// A stage names a step of *answering*, and it would name the same steps if the agent were
/// three plain functions in a row. That is what keeps this from being the agent's internals
/// leaking onto the wire: the set is closed here, and the agent maps its own steps onto it
/// explicitly rather than deriving a name from whatever it happens to call a function today.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    /// Reading what it was handed.
    Gathering,
    /// Asking the model.
    Thinking,
    /// Turning an answer into what was asked for.
    Shaping,
    /// Waiting on the application to answer a [`super::message::EventBody::ToolCall`].
    Calling,
}

/// A capability the application is willing to perform on a run's behalf.
///
/// **There are none, and that is the design rather than an omission.** Nothing in the
/// application executes anything yet, so there is no capability to name; an empty set means
/// every `tool_call` is refused by this type before it can be encoded, and the day a capability
/// is agreed it is agreed on both sides in one change.
///
/// What is being written down now is the decision the protocol exists to record: **the agent
/// asks, and the application executes.** An agent never runs anything itself, and a
/// [`Suggestion`] — which is prose and inert — is a different object with a different name,
/// deliberately not this one.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolName {}

/// Who said one turn of a conversation.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// The person using the application.
    Person,
    /// The agent.
    Agent,
}

/// One turn of the conversation handed to a run.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Turn {
    /// Who said it.
    pub role: Role,
    /// What was said.
    pub content: String,
}

/// An inert description of something a person might do.
///
/// **Inert is the whole of it.** This names no command, no path, no tool and nothing else that
/// could be executed, and it never will: the object that asks for something to happen is
/// [`super::message::EventBody::ToolCall`], and keeping the two apart is what stops a model's
/// prose from becoming an instruction. A field added here that a caller could act on is the
/// attack this type exists to refuse.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Suggestion {
    /// One line naming the suggestion.
    pub title: String,
    /// What is being suggested, in prose.
    pub body: String,
    /// The names of the resources the agent was handed. Not paths, and nothing to open.
    #[serde(default)]
    pub sources: Vec<String>,
}

/// Why a run could not be answered.
///
/// **Not an enum, and this is the one vocabulary of the protocol that is deliberately open.**
/// An agent built after this application must be able to say why it failed without waiting for
/// a release of Rust to carry the word, so a code is a string this side stores and hands on.
/// Nothing here matches exhaustively; the interface narrows it against its own catalogs and
/// draws a generic sentence for a code it has never heard of.
///
/// It is the same decision `preferences.rs` in the application already documents, for the same
/// reason: the side that knows what a value *means* is the side that should hold the list.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct ErrorCode(Cow<'static, str>);

impl ErrorCode {
    /// The frame was not a MessagePack map.
    pub const MESSAGE_NOT_DECODABLE: Self = Self(Cow::Borrowed("message_not_decodable"));
    /// The frame decoded, and was not a message of this contract.
    pub const MESSAGE_NOT_UNDERSTOOD: Self = Self(Cow::Borrowed("message_not_understood"));
    /// The frame named a contract version this build does not speak.
    pub const CONTRACT_VERSION_UNSUPPORTED: Self =
        Self(Cow::Borrowed("contract_version_unsupported"));
    /// A length prefix named more bytes than this build will read.
    pub const FRAME_TOO_LARGE: Self = Self(Cow::Borrowed("frame_too_large"));
    /// A resource was asked for by a name the agent does not hold.
    pub const RESOURCE_UNKNOWN: Self = Self(Cow::Borrowed("resource_unknown"));
    /// Nothing answered at the model's endpoint.
    pub const MODEL_UNREACHABLE: Self = Self(Cow::Borrowed("model_unreachable"));
    /// The endpoint answered and does not serve the model it was asked for.
    pub const MODEL_UNKNOWN: Self = Self(Cow::Borrowed("model_unknown"));
    /// A run arrived while one was already in flight. Carries the identifier of the new one.
    pub const RUN_ALREADY_IN_FLIGHT: Self = Self(Cow::Borrowed("run_already_in_flight"));
    /// A message named a run that is not in flight.
    pub const RUN_UNKNOWN: Self = Self(Cow::Borrowed("run_unknown"));
    /// A tool result arrived for a call that was never issued, or that is no longer awaited.
    pub const TOOL_RESULT_UNEXPECTED: Self = Self(Cow::Borrowed("tool_result_unexpected"));
    /// A run tried to call a capability its command did not offer.
    pub const TOOL_NOT_OFFERED: Self = Self(Cow::Borrowed("tool_not_offered"));
    /// The agent was asked for and would not start.
    pub const AGENT_WILL_NOT_START: Self = Self(Cow::Borrowed("agent_will_not_start"));
    /// The agent stopped while a run was in flight.
    pub const AGENT_STOPPED: Self = Self(Cow::Borrowed("agent_stopped"));

    /// A code this build has no constant for, as it arrived.
    ///
    /// Used when forwarding a code from an agent newer than this application. Prefer a
    /// constant above wherever one exists.
    #[must_use]
    pub fn forwarded(code: String) -> Self {
        Self(Cow::Owned(code))
    }

    /// The code, as it travels.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
