//! One run, and the events it produces on their way to a screen.
//!
//! Two types cross this module and they are deliberately not the same one. What arrives is a
//! [`Event`] of the Agent Protocol, which carries a contract version and is the business of two
//! programs; what leaves is an [`AgentEvent`], which is the business of a screen. Mapping
//! between them is the whole of this file, and it is what stops the protocol crate's public
//! surface from reaching the webview — a screen that read the wire type would have to be
//! changed by every change to the wire.

use almena_agent_protocol::message::{Event, EventBody};
use almena_agent_protocol::vocabulary::{ErrorCode, Stage};
use serde::Serialize;
use tauri::ipc::Channel;

/// What a screen is told about a run, as it happens.
///
/// Every variant carries what a screen can draw and nothing else. There is no contract version
/// here, no identifier and no `detail`: the channel belongs to one run, so nothing has to be
/// correlated, and prose from a subprocess is never drawn — `.agents/rules/language.md`
/// says what a person reads comes from the catalogs, looked up by the identifier below.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "event", rename_all = "camelCase")]
pub enum AgentEvent {
    /// The agent has the run, and has produced nothing yet.
    ///
    /// It exists so that *nobody has looked yet* and *it is working* stay two facts on the
    /// screen as well as on the wire — `.agents/rules/honest-emptiness.md`.
    Started,
    /// A stage of answering began, or moved.
    Progress {
        /// Which stage, as an identifier the interface translates.
        stage: Stage,
        /// How much is done, or `null` where nothing counted it. Never `0` for unknown.
        done: Option<u32>,
        /// How much there is, or `null` where nothing counted it.
        total: Option<u32>,
    },
    /// One piece of the answer, in the order it was produced.
    Token {
        /// The piece.
        text: String,
    },
    /// The one answer to a run that asked for a suggestion.
    Proposal {
        /// One line naming it.
        title: String,
        /// What is being suggested, in prose.
        body: String,
        /// The resources the agent was handed. Names, not paths, and nothing to open.
        sources: Vec<String>,
    },
    /// The run finished. Terminal.
    Completed,
    /// The run was stopped because somebody asked. Terminal.
    ///
    /// Not a retraction: whatever arrived before it stands, and a screen that cleared the
    /// answer would be throwing away something a person has already read.
    Cancelled,
    /// The run did not finish, and this is why. Terminal.
    Failed {
        /// An identifier the interface looks up. Never prose.
        code: String,
    },
}

impl AgentEvent {
    /// Whether this ends the run, after which the channel is closed and nothing more is sent.
    #[must_use]
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Cancelled | Self::Failed { .. }
        )
    }

    /// A failure carrying one identifier.
    #[must_use]
    pub fn failed(code: &ErrorCode) -> Self {
        Self::Failed {
            code: code.as_str().to_owned(),
        }
    }
}

/// What the agent said about itself when it started.
///
/// Both fields are what the **running** agent reported, which is a different fact from what was
/// chosen — and the two are worth showing separately, because a model that was asked for and a
/// model that is in force differ every time somebody changes the setting and has not restarted.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Ready {
    /// What the agent calls itself.
    pub agent_version: String,
    /// The model it was started with, or `None` where it was told nothing.
    pub model: Option<String>,
}

/// The run in flight, and where its events are going.
pub struct Exchange {
    /// The identifier this side gave it. Every event about it carries the same one.
    pub id: String,
    /// Where a screen is listening.
    pub channel: Channel<AgentEvent>,
}

impl Exchange {
    /// Sends one event to the screen, saying whether it was the last.
    ///
    /// A send that fails means the webview went — a reloaded window, a closed one — which is
    /// not worth ending a run over and not worth a record at `ERROR`.
    pub fn tell(&self, event: &AgentEvent) -> bool {
        if self.channel.send(event.clone()).is_err() {
            log::debug!("agent_event_not_delivered exchange={}", self.id);
        }
        event.is_terminal()
    }
}

/// What a screen should be told about one event of the protocol, if anything.
///
/// `None` for an event that is not about a run in flight: [`EventBody::Ready`] is about the
/// agent rather than about anything anybody asked for, and [`EventBody::ToolCall`] cannot
/// arrive at all, because no capability has been agreed and the contract refuses every name.
#[must_use]
pub fn about(event: Event) -> Option<(Option<String>, AgentEvent)> {
    match event.body {
        // About the agent rather than about anything anybody asked for.
        EventBody::Ready { .. } => None,
        EventBody::Started { id } => Some((Some(id), AgentEvent::Started)),
        EventBody::Progress {
            id,
            stage,
            done,
            total,
        } => Some((Some(id), AgentEvent::Progress { stage, done, total })),
        EventBody::Token { id, text } => Some((Some(id), AgentEvent::Token { text })),
        EventBody::Proposal { id, suggestion } => Some((
            Some(id),
            AgentEvent::Proposal {
                title: suggestion.title,
                body: suggestion.body,
                sources: suggestion.sources,
            },
        )),
        EventBody::Completed { id } => Some((Some(id), AgentEvent::Completed)),
        EventBody::Cancelled { id } => Some((Some(id), AgentEvent::Cancelled)),
        EventBody::Failed { id, code, detail } => {
            // `detail` is English written for a log, and this is the one place it is read. It
            // goes no further: what a person sees comes from `code`, through the catalogs.
            log::warn!("agent_run_failed code={} detail={detail}", code.as_str());
            Some((id, AgentEvent::failed(&code)))
        }
        // Unreachable while no capability is agreed — the contract refuses every name there
        // is. Refused rather than dropped, so that a build which somehow produced one fails
        // in front of somebody instead of hanging a run that will never be answered.
        EventBody::ToolCall { id, .. } => {
            Some((Some(id), AgentEvent::failed(&ErrorCode::TOOL_NOT_OFFERED)))
        }
    }
}

/// What the agent said about itself, or nothing when the event was about a run.
#[must_use]
pub fn ready_of(event: &Event) -> Option<Ready> {
    match &event.body {
        EventBody::Ready {
            agent_version,
            model,
        } => Some(Ready {
            agent_version: agent_version.clone(),
            model: model.clone(),
        }),
        _ => None,
    }
}
