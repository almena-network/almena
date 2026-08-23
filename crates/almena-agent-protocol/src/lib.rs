//! The Agent Protocol: what the application and an agent say to each other.
//!
//! An agent is a program of its own, in a repository of its own, in a language of its own. This
//! crate is the half of the contract between them that Rust holds — the messages, and how one
//! becomes bytes on a pipe. It is deliberately the *whole* of what the application knows about
//! an agent: **nothing here names a graph, a model, a framework or a library**, so the agent
//! that speaks it today can be replaced by one written in Rust, or compiled to WASM, without a
//! word of this changing.
//!
//! That is the argument for it being a crate rather than a module of the application. It is not
//! here to share code — one program links it — it is here because a contract with an edition,
//! whose other half is in somebody else's repository, is a decision, and a decision is easier
//! to hold to in one place with a version number on it. `almena-log` owns the shape of a
//! record and `almena-paths` owns where a program keeps things; this owns what two programs
//! may say to each other.
//!
//! It is also the only arrangement in which `task isolation` can assert anything about it:
//! `cargo tree -p almena-agent-protocol -i tauri` needs a package to ask about, and a contract
//! that had quietly grown a `tauri::ipc::Channel` in one of its types is a contract no other
//! runtime could link.
//!
//! # Where the transport is, and is not
//!
//! [`framing`] knows about `Read` and `Write` and nothing else. There is no process here, no
//! pipe, no spawn and no supervision: which program is at the other end, and who started it, is
//! the application's business. See `src-tauri/src/agent/` for that half.
//!
//! # Four facts about the encoding, established rather than assumed
//!
//! These were measured against `rmp-serde` before anything was built on them, because each is a
//! way two languages can disagree in silence:
//!
//! - A message is encoded as a **map with string keys**, via `to_vec_named`. The default
//!   encodes a struct as an array, which would make field order load-bearing across two
//!   languages.
//! - A key this build does not know is **ignored**. That is the forward-compatible direction and
//!   it is deliberate: an agent newer than this application may add a field to an event, and
//!   the application should carry on rather than refuse the whole run. The agent is strict in
//!   the other direction, which is the correct asymmetry — the side receiving instructions
//!   should be the fussy one.
//! - A missing optional key decodes as absent rather than as an error.
//! - An **unrecognised `command` or `event` is refused.** A message this build has no name for
//!   is not silently dropped, which is the one case where being lax would lose a whole run.
//!
//! # The vocabulary of a failure is open, and everything else is closed
//!
//! [`vocabulary::ErrorCode`] is a string, so an agent can say why it failed without waiting for
//! a release of this crate to carry the word. Everything else — the intents, the stages, the
//! capabilities — is an enum that refuses anything not listed, because those are things the two
//! sides have to have agreed on in advance to mean anything at all.

pub mod framing;
pub mod message;
pub mod vocabulary;

pub use framing::{MAX_FRAME_BYTES, PREFIX_BYTES, ProtocolError};
pub use message::{CONTRACT_VERSION, Command, CommandBody, Event, EventBody, Params};
pub use vocabulary::{ErrorCode, Intent, Role, Stage, Suggestion, ToolName, Turn};
