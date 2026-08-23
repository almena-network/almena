//! The frames both implementations of this contract must agree on, byte for byte.
//!
//! The agent is a program in another language, in another repository, released on its own
//! schedule. Nothing about the two being written by the same people makes them agree — what
//! makes them agree is a corpus of exact bytes that each one is held to, and this is Rust's
//! copy of it.
//!
//! **The twin lives at `agent/tests/test_golden_frames.py`, and the two must carry the same
//! table.** Nothing enforces that across two repositories; `TODO.md` says so rather than
//! leaving it to be discovered. Changing a byte here without changing it there is the failure
//! this file exists to make loud, and the day it happens the symptom is an agent that starts,
//! says nothing a person can see, and writes one refusal to a log.
//!
//! # What each side promises
//!
//! Rust owns the canonical order, so it is held to **both** directions: it decodes every frame
//! below to the message named, and re-encodes that message to exactly those bytes.
//!
//! Python is held to decoding them. It is deliberately *not* held to reproducing the bytes,
//! because a MessagePack map is unordered and a second implementation emitting its keys in
//! another order is correct rather than broken — Rust ignores order when it reads. Requiring
//! byte-equality in that direction would be requiring something the format does not mean.

// Every function here is part of a test, including the two helpers, which clippy cannot tell
// because an integration test's helpers carry no `#[test]` of their own. A corpus that cannot
// be read is a failing test, which is exactly what a panic here is.
#![allow(clippy::expect_used, reason = "the whole of this file is a test")]

use almena_agent_protocol::framing::{decode_command, decode_event};
use almena_agent_protocol::message::{
    CONTRACT_VERSION, Command, CommandBody, Event, EventBody, Params,
};
use almena_agent_protocol::vocabulary::{ErrorCode, Intent, Role, Stage, Suggestion, Turn};

/// One frame of the corpus: what it is called, its payload, and what it means.
struct Golden<T> {
    name: &'static str,
    hex: &'static str,
    message: T,
}

/// The bytes a hex string carries.
fn bytes_of(hex: &str) -> Vec<u8> {
    assert!(hex.len().is_multiple_of(2), "{hex} is not whole bytes");
    (0..hex.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&hex[at..at + 2], 16).expect("the corpus is hexadecimal"))
        .collect()
}

/// The hex a run of bytes is written as.
fn hex_of(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn commands() -> Vec<Golden<Command>> {
    vec![
        Golden {
            name: "run_chat",
            hex: "85b0636f6e74726163745f76657273696f6ea132a7636f6d6d616e64a372756ea26964a137a6696e74656e74a463686174a6706172616d7383a86d657373616765739182a4726f6c65a6706572736f6ea7636f6e74656e74a4686f6c61a97265736f757263657391aa616c6d656e612e747874a5746f6f6c7390",
            message: Command::new(CommandBody::Run {
                id: "7".to_owned(),
                intent: Intent::Chat,
                params: Params {
                    messages: vec![Turn {
                        role: Role::Person,
                        content: "hola".to_owned(),
                    }],
                    resources: vec!["almena.txt".to_owned()],
                    tools: Vec::new(),
                },
            }),
        },
        Golden {
            name: "cancel",
            hex: "83b0636f6e74726163745f76657273696f6ea132a7636f6d6d616e64a663616e63656ca26964a137",
            message: Command::new(CommandBody::Cancel { id: "7".to_owned() }),
        },
        Golden {
            name: "tool_result_declined",
            hex: "85b0636f6e74726163745f76657273696f6ea132a7636f6d6d616e64ab746f6f6c5f726573756c74a26964a137a763616c6c5f6964a26331a66f7574707574c0",
            message: Command::new(CommandBody::ToolResult {
                id: "7".to_owned(),
                call_id: "c1".to_owned(),
                output: None,
            }),
        },
    ]
}

/// One event, in the contract version this build speaks.
fn event(body: EventBody) -> Event {
    Event {
        contract_version: CONTRACT_VERSION.to_owned(),
        body,
    }
}

/// Every frame of the corpus that is about a run existing rather than about what it produced.
fn lifecycle_events() -> Vec<Golden<Event>> {
    vec![
        Golden {
            name: "ready",
            hex: "84b0636f6e74726163745f76657273696f6ea132a56576656e74a57265616479ad6167656e745f76657273696f6ea5302e312e30a56d6f64656ca567656d6d61",
            message: event(EventBody::Ready {
                agent_version: "0.1.0".to_owned(),
                model: Some("gemma".to_owned()),
            }),
        },
        Golden {
            name: "started",
            hex: "83b0636f6e74726163745f76657273696f6ea132a56576656e74a773746172746564a26964a137",
            message: event(EventBody::Started { id: "7".to_owned() }),
        },
        Golden {
            // The one worth having in a corpus: a stage nobody counted. `done` and `total` are
            // present and nil — `c0` — and not absent, and not `0`. A zero here would be a
            // measurement claimed by an implementation that took none.
            name: "progress_unmeasured",
            hex: "86b0636f6e74726163745f76657273696f6ea132a56576656e74a870726f6772657373a26964a137a57374616765a87468696e6b696e67a4646f6e65c0a5746f74616cc0",
            message: event(EventBody::Progress {
                id: "7".to_owned(),
                stage: Stage::Thinking,
                done: None,
                total: None,
            }),
        },
    ]
}

/// Every frame of the corpus that carries something a run produced.
fn content_events() -> Vec<Golden<Event>> {
    vec![
        Golden {
            name: "token",
            hex: "84b0636f6e74726163745f76657273696f6ea132a56576656e74a5746f6b656ea26964a137a474657874a4686f6c61",
            message: event(EventBody::Token {
                id: "7".to_owned(),
                text: "hola".to_owned(),
            }),
        },
        Golden {
            name: "proposal",
            hex: "84b0636f6e74726163745f76657273696f6ea132a56576656e74a870726f706f73616ca26964a137aa73756767657374696f6e83a57469746c65a36f6e65a4626f6479a374776fa7736f757263657391aa616c6d656e612e747874",
            message: event(EventBody::Proposal {
                id: "7".to_owned(),
                suggestion: Suggestion {
                    title: "one".to_owned(),
                    body: "two".to_owned(),
                    sources: vec!["almena.txt".to_owned()],
                },
            }),
        },
    ]
}

/// Every frame of the corpus that ends a run.
fn terminal_events() -> Vec<Golden<Event>> {
    vec![
        Golden {
            name: "completed",
            hex: "83b0636f6e74726163745f76657273696f6ea132a56576656e74a9636f6d706c65746564a26964a137",
            message: event(EventBody::Completed { id: "7".to_owned() }),
        },
        Golden {
            name: "cancelled",
            hex: "83b0636f6e74726163745f76657273696f6ea132a56576656e74a963616e63656c6c6564a26964a137",
            message: event(EventBody::Cancelled { id: "7".to_owned() }),
        },
        Golden {
            name: "failed_unattributed",
            hex: "85b0636f6e74726163745f76657273696f6ea132a56576656e74a66661696c6564a26964c0a4636f6465af6672616d655f746f6f5f6c61726765a664657461696ca178",
            message: event(EventBody::Failed {
                id: None,
                code: ErrorCode::FRAME_TOO_LARGE,
                detail: "x".to_owned(),
            }),
        },
    ]
}

#[test]
fn every_golden_command_decodes_to_what_it_says_it_is() {
    for golden in commands() {
        let decoded = decode_command(&bytes_of(golden.hex))
            .unwrap_or_else(|error| panic!("{}: {error}", golden.name));
        assert_eq!(decoded, golden.message, "{}", golden.name);
    }
}

#[test]
fn every_golden_event_decodes_to_what_it_says_it_is() {
    for golden in events() {
        let decoded = decode_event(&bytes_of(golden.hex))
            .unwrap_or_else(|error| panic!("{}: {error}", golden.name));
        assert_eq!(decoded, golden.message, "{}", golden.name);
    }
}

#[test]
fn every_golden_command_is_encoded_back_to_exactly_its_own_bytes() {
    for golden in commands() {
        let encoded = rmp_serde::to_vec_named(&golden.message).expect("a command encodes");
        assert_eq!(hex_of(&encoded), golden.hex, "{}", golden.name);
    }
}

#[test]
fn every_golden_event_is_encoded_back_to_exactly_its_own_bytes() {
    for golden in events() {
        let encoded = rmp_serde::to_vec_named(&golden.message).expect("an event encodes");
        assert_eq!(hex_of(&encoded), golden.hex, "{}", golden.name);
    }
}

/// The whole corpus of events, in the three groups it falls into.
fn events() -> Vec<Golden<Event>> {
    let mut all = lifecycle_events();
    all.extend(content_events());
    all.extend(terminal_events());
    all
}
