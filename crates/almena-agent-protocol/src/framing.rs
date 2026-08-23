//! How a message becomes bytes on a pipe, and back.
//!
//! # The frame
//!
//! ```text
//! ┌──────────────────────────┬────────────────────────────────┐
//! │ u32, big-endian, 4 bytes │ one MessagePack map, N bytes   │
//! └──────────────────────────┴────────────────────────────────┘
//! ```
//!
//! The prefix counts **the payload only**, not itself. That sentence is here because it is the
//! one thing two implementations in two languages get wrong in opposite directions.
//!
//! Newlines cannot frame a binary encoding — MessagePack contains `0x0A` freely — so the
//! length goes in front. MessagePack is self-delimiting and the prefix is therefore redundant,
//! and it buys three things anyway: a reader can hand a **complete slice** to a decoder that
//! needs no `Read` and is trivial to test, an allocation is bounded before anything is parsed,
//! and a stream that has lost its place can be diagnosed without a MessagePack parser in hand.
//! Four bytes is a cheap price for all three.
//!
//! Big-endian because MessagePack's own integers are, so one pipe has one convention.
//!
//! # Past the limit there is no recovery
//!
//! A prefix naming more than [`MAX_FRAME_BYTES`] is fatal. The reader cannot skip N bytes,
//! because N is the number it just refused to trust — skipping it would be trusting it. So it
//! stops, and whoever owns the child process ends it and starts another. Inventing a
//! resynchronisation would be inventing a parser, which is the thing this module exists not to
//! have.
//!
//! # Two passes, so that the version wins
//!
//! Decoding reads `contract_version` on its own first and only then decodes the message. It
//! costs a second pass over a few hundred bytes and it buys the right error: a frame from a
//! contract this build does not speak is refused as **that**, rather than as whichever field
//! happened to have changed shape.

use std::io::{Read, Write};

use serde::Deserialize;

use crate::message::{CONTRACT_VERSION, Command, Event};
use crate::vocabulary::ErrorCode;

/// How many bytes the length prefix occupies.
pub const PREFIX_BYTES: usize = 4;

/// The largest payload this build will read or write, in bytes.
///
/// Eight mebibytes. Tokens are bytes and conversations are kilobytes; the only large thing a
/// frame can carry is a run's handed-over text, and the application decides how much of that
/// there is. Far past anything real, and far short of a prefix — corrupt or hostile — being
/// able to ask for an allocation that matters.
///
/// A constant of this crate rather than an argument, for the reason `almena_log::MAX_FILE_SIZE`
/// is one: two programs that could be configured to disagree eventually will.
pub const MAX_FRAME_BYTES: u32 = 8 * 1024 * 1024;

/// Why a frame could not be read, written or understood.
#[derive(Debug)]
pub enum ProtocolError {
    /// The payload was not a MessagePack map.
    NotDecodable(String),
    /// The payload decoded, and was not a message of this contract.
    NotUnderstood(String),
    /// The frame named a contract version this build does not speak.
    VersionUnsupported {
        /// What it named.
        named: String,
    },
    /// A length prefix named more bytes than this build will read or write.
    FrameTooLarge {
        /// How many it named.
        bytes: u32,
    },
    /// The input ended part-way through a frame.
    Truncated,
    /// The pipe itself failed.
    Io(std::io::Error),
}

impl ProtocolError {
    /// The identifier a reader is told, for the failures worth telling one about.
    #[must_use]
    pub fn code(&self) -> ErrorCode {
        match self {
            Self::NotDecodable(_) => ErrorCode::MESSAGE_NOT_DECODABLE,
            Self::NotUnderstood(_) => ErrorCode::MESSAGE_NOT_UNDERSTOOD,
            Self::VersionUnsupported { .. } => ErrorCode::CONTRACT_VERSION_UNSUPPORTED,
            Self::FrameTooLarge { .. } => ErrorCode::FRAME_TOO_LARGE,
            // Neither is the far side's doing, and neither is a thing it can be told: the pipe
            // it would be told over is the one that just failed.
            Self::Truncated | Self::Io(_) => ErrorCode::AGENT_STOPPED,
        }
    }
}

impl std::fmt::Display for ProtocolError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotDecodable(detail) => write!(formatter, "frame is not messagepack: {detail}"),
            Self::NotUnderstood(detail) => write!(formatter, "frame is not a message: {detail}"),
            Self::VersionUnsupported { named } => {
                write!(
                    formatter,
                    "contract_version={named} speaks={CONTRACT_VERSION}"
                )
            }
            Self::FrameTooLarge { bytes } => {
                write!(formatter, "frame_bytes={bytes} maximum={MAX_FRAME_BYTES}")
            }
            Self::Truncated => write!(formatter, "the input ended part-way through a frame"),
            Self::Io(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for ProtocolError {}

impl From<std::io::Error> for ProtocolError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

/// Just enough of any frame to check the version before anything else is looked at.
#[derive(Deserialize)]
struct Envelope {
    contract_version: String,
}

/// Just enough of any frame to attribute a failure to a run.
#[derive(Deserialize)]
struct Attribution {
    #[serde(default)]
    id: Option<String>,
}

/// One command, framed and ready for a pipe.
///
/// # Errors
///
/// Returns [`ProtocolError::NotUnderstood`] when the command cannot be encoded, and
/// [`ProtocolError::FrameTooLarge`] when it encodes to more than [`MAX_FRAME_BYTES`] — in which
/// case **nothing has been written anywhere**, which is what keeps a refusal from desynchronising
/// a stream.
pub fn encode(command: &Command) -> Result<Vec<u8>, ProtocolError> {
    // `to_vec_named` and not `to_vec`: the default encodes a struct as an array, which makes
    // field order load-bearing across two languages and a hex dump unreadable.
    let payload = rmp_serde::to_vec_named(command)
        .map_err(|error| ProtocolError::NotUnderstood(error.to_string()))?;

    let length = u32::try_from(payload.len())
        .map_err(|_| ProtocolError::FrameTooLarge { bytes: u32::MAX })?;
    if length > MAX_FRAME_BYTES {
        return Err(ProtocolError::FrameTooLarge { bytes: length });
    }

    let mut frame = Vec::with_capacity(PREFIX_BYTES + payload.len());
    frame.extend_from_slice(&length.to_be_bytes());
    frame.extend_from_slice(&payload);
    Ok(frame)
}

/// Writes one command and flushes it, because the far side is waiting on it.
///
/// # Errors
///
/// Whatever [`encode`] returns, or [`ProtocolError::Io`] when the pipe failed.
pub fn write<W: Write>(sink: &mut W, command: &Command) -> Result<(), ProtocolError> {
    let frame = encode(command)?;
    sink.write_all(&frame)?;
    sink.flush()?;
    Ok(())
}

/// Reads one payload, or `None` where the input ended cleanly between frames.
///
/// `None` is the far side having gone, which is the ordinary way this ends and not a failure.
///
/// # Errors
///
/// [`ProtocolError::FrameTooLarge`] when the prefix names more than [`MAX_FRAME_BYTES`], after
/// which **this reader must not be used again** — the stream's position is no longer known.
/// [`ProtocolError::Truncated`] when the input ended part-way through a frame, and
/// [`ProtocolError::Io`] when the pipe failed.
pub fn read<R: Read>(source: &mut R) -> Result<Option<Vec<u8>>, ProtocolError> {
    let mut prefix = [0_u8; PREFIX_BYTES];
    match source.read_exact(&mut prefix) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(error) => return Err(ProtocolError::Io(error)),
    }

    let length = u32::from_be_bytes(prefix);
    if length > MAX_FRAME_BYTES {
        return Err(ProtocolError::FrameTooLarge { bytes: length });
    }

    let mut payload = vec![0_u8; length as usize];
    match source.read_exact(&mut payload) {
        Ok(()) => Ok(Some(payload)),
        // A prefix arrived and its payload did not. That is not a clean ending: somebody was
        // part-way through saying something.
        Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => {
            Err(ProtocolError::Truncated)
        }
        Err(error) => Err(ProtocolError::Io(error)),
    }
}

/// The command one payload carries.
///
/// # Errors
///
/// [`ProtocolError::NotDecodable`] when the payload is not MessagePack,
/// [`ProtocolError::VersionUnsupported`] when it names another contract — checked first — and
/// [`ProtocolError::NotUnderstood`] when it is neither of the three commands.
pub fn decode_command(payload: &[u8]) -> Result<Command, ProtocolError> {
    check_version(payload)?;
    rmp_serde::from_slice(payload).map_err(|error| ProtocolError::NotUnderstood(error.to_string()))
}

/// The event one payload carries.
///
/// # Errors
///
/// The same three as [`decode_command`], in the same order.
pub fn decode_event(payload: &[u8]) -> Result<Event, ProtocolError> {
    check_version(payload)?;
    rmp_serde::from_slice(payload).map_err(|error| ProtocolError::NotUnderstood(error.to_string()))
}

/// Refuses a payload from a contract this build does not speak, before anything else reads it.
///
/// # Errors
///
/// [`ProtocolError::NotDecodable`] when the payload is not a MessagePack map at all, and
/// [`ProtocolError::VersionUnsupported`] when it names a version other than
/// [`CONTRACT_VERSION`].
fn check_version(payload: &[u8]) -> Result<(), ProtocolError> {
    let envelope: Envelope = rmp_serde::from_slice(payload)
        .map_err(|error| ProtocolError::NotDecodable(error.to_string()))?;

    if envelope.contract_version == CONTRACT_VERSION {
        Ok(())
    } else {
        Err(ProtocolError::VersionUnsupported {
            named: envelope.contract_version,
        })
    }
}

/// The run a payload names, for attributing a failure to one when it could not be decoded.
///
/// Answers `None` whenever the payload does not plainly carry one. A frame that never had an
/// identifier is not made to yield one.
#[must_use]
pub fn identifier_of(payload: &[u8]) -> Option<String> {
    rmp_serde::from_slice::<Attribution>(payload)
        .ok()
        .and_then(|found| found.id)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{
        MAX_FRAME_BYTES, PREFIX_BYTES, ProtocolError, decode_command, decode_event, encode,
        identifier_of, read,
    };
    use crate::message::{CONTRACT_VERSION, Command, CommandBody, Event, EventBody, Params};
    use crate::vocabulary::{ErrorCode, Intent, Role, Stage, Suggestion, Turn};

    /// The payload of a frame, without its prefix.
    fn payload_of(frame: &[u8]) -> &[u8] {
        &frame[PREFIX_BYTES..]
    }

    /// A frame carrying an event, as the agent would write one.
    fn event_frame(body: EventBody) -> Vec<u8> {
        let event = Event {
            contract_version: CONTRACT_VERSION.to_owned(),
            body,
        };
        let payload = rmp_serde::to_vec_named(&event).expect("an event encodes");
        let length = u32::try_from(payload.len()).expect("a test frame is small");
        let mut frame = length.to_be_bytes().to_vec();
        frame.extend_from_slice(&payload);
        frame
    }

    #[test]
    fn a_frame_says_its_own_length_before_its_bytes() {
        let frame = encode(&Command::new(CommandBody::Cancel { id: "7".to_owned() }))
            .expect("a cancel encodes");

        let named = u32::from_be_bytes(
            frame[..PREFIX_BYTES]
                .try_into()
                .expect("the prefix is four bytes"),
        );

        // The prefix counts the payload and not itself. This is the assertion that catches the
        // off-by-four, and it is the one thing two languages get wrong in opposite directions.
        assert_eq!(named as usize, frame.len() - PREFIX_BYTES);
    }

    #[test]
    fn two_frames_in_one_read_are_two_messages() {
        // The first carries a newline and the second carries four bytes that look exactly like
        // a length prefix. Either would defeat a reader that framed on `0x0A` or that went
        // looking for a boundary instead of counting to one.
        let first = encode(&Command::new(CommandBody::Run {
            id: "7".to_owned(),
            intent: Intent::Chat,
            params: Params {
                messages: vec![Turn {
                    role: Role::Person,
                    content: "one\ntwo\nthree".to_owned(),
                }],
                ..Params::default()
            },
        }))
        .expect("a run encodes");

        let second = encode(&Command::new(CommandBody::ToolResult {
            id: "8".to_owned(),
            call_id: "c1".to_owned(),
            output: Some("\u{0}\u{0}\u{1}\u{2}".to_owned()),
        }))
        .expect("a tool result encodes");

        let mut both = first.clone();
        both.extend_from_slice(&second);
        let mut source = both.as_slice();

        let one = read(&mut source)
            .expect("the first reads")
            .expect("not eof");
        let two = read(&mut source)
            .expect("the second reads")
            .expect("not eof");

        assert_eq!(one, payload_of(&first));
        assert_eq!(two, payload_of(&second));
        assert!(
            read(&mut source).expect("the input ends").is_none(),
            "the two frames were the whole of the input"
        );
    }

    #[test]
    fn a_prefix_larger_than_the_maximum_is_refused_before_the_payload_is_read() {
        // A prefix and nothing behind it. A reader that trusted the number would sit here
        // waiting for eight mebibytes that are never coming.
        let frame = (MAX_FRAME_BYTES + 1).to_be_bytes();
        let mut source = frame.as_slice();

        let refused = read(&mut source).expect_err("a prefix past the maximum is refused");
        assert!(
            matches!(refused, ProtocolError::FrameTooLarge { bytes } if bytes == MAX_FRAME_BYTES + 1)
        );
        assert_eq!(refused.code(), ErrorCode::FRAME_TOO_LARGE);
    }

    #[test]
    fn an_input_that_ends_between_frames_is_not_a_failure() {
        let mut source: &[u8] = &[];
        assert!(
            read(&mut source)
                .expect("an empty input is an ending")
                .is_none()
        );
    }

    #[test]
    fn an_input_that_ends_inside_a_frame_is_a_failure() {
        let frame = encode(&Command::new(CommandBody::Cancel { id: "7".to_owned() }))
            .expect("a cancel encodes");
        let mut source = &frame[..frame.len() - 1];

        assert!(matches!(
            read(&mut source).expect_err("a truncated frame is refused"),
            ProtocolError::Truncated
        ));
    }

    #[test]
    fn every_command_round_trips_through_the_wire_unchanged() {
        for body in [
            CommandBody::Run {
                id: "7".to_owned(),
                intent: Intent::Chat,
                params: Params::default(),
            },
            CommandBody::Run {
                id: "7".to_owned(),
                intent: Intent::Propose,
                params: Params {
                    messages: vec![Turn {
                        role: Role::Agent,
                        content: "hola".to_owned(),
                    }],
                    resources: vec!["almena.txt".to_owned()],
                    tools: Vec::new(),
                },
            },
            CommandBody::Cancel { id: "7".to_owned() },
            CommandBody::ToolResult {
                id: "7".to_owned(),
                call_id: "c1".to_owned(),
                output: None,
            },
        ] {
            let command = Command::new(body);
            let frame = encode(&command).expect("a command encodes");
            let back = decode_command(payload_of(&frame)).expect("a command decodes");
            assert_eq!(back, command);
        }
    }

    #[test]
    fn every_event_round_trips_through_the_wire_unchanged() {
        for body in [
            EventBody::Ready {
                agent_version: "0.1.0".to_owned(),
                model: Some("gemma".to_owned()),
            },
            EventBody::Ready {
                agent_version: "0.1.0".to_owned(),
                model: None,
            },
            EventBody::Started { id: "7".to_owned() },
            EventBody::Progress {
                id: "7".to_owned(),
                stage: Stage::Gathering,
                done: Some(1),
                total: Some(3),
            },
            // The one that matters: a stage nobody counted says so with absence, never with a
            // zero, and the absence has to survive the wire.
            EventBody::Progress {
                id: "7".to_owned(),
                stage: Stage::Thinking,
                done: None,
                total: None,
            },
            EventBody::Token {
                id: "7".to_owned(),
                text: "hola".to_owned(),
            },
            EventBody::Proposal {
                id: "7".to_owned(),
                suggestion: Suggestion {
                    title: "one".to_owned(),
                    body: "two".to_owned(),
                    sources: vec!["almena.txt".to_owned()],
                },
            },
            EventBody::Completed { id: "7".to_owned() },
            EventBody::Cancelled { id: "7".to_owned() },
            EventBody::Failed {
                id: None,
                code: ErrorCode::FRAME_TOO_LARGE,
                detail: "frame_bytes=9000000".to_owned(),
            },
        ] {
            let frame = event_frame(body.clone());
            let back = decode_event(payload_of(&frame)).expect("an event decodes");
            assert_eq!(back.body, body);
            assert_eq!(back.contract_version, CONTRACT_VERSION);
        }
    }

    #[test]
    fn a_frame_of_a_contract_this_build_does_not_speak_is_refused_as_that() {
        // Both wrong at once: a version nobody speaks, and a command nobody has. The version
        // has to win, or a reader is told to fix the wrong thing.
        let mut map: BTreeMap<&str, &str> = BTreeMap::new();
        map.insert("contract_version", "99");
        map.insert("command", "invented");
        let payload = rmp_serde::to_vec_named(&map).expect("a map encodes");

        let refused = decode_command(&payload).expect_err("another contract is refused");
        assert!(matches!(
            refused,
            ProtocolError::VersionUnsupported { ref named } if named == "99"
        ));
        assert_eq!(refused.code(), ErrorCode::CONTRACT_VERSION_UNSUPPORTED);
    }

    #[test]
    fn a_payload_that_is_not_messagepack_is_refused() {
        let refused = decode_event(b"not messagepack at all").expect_err("rubbish is refused");
        assert_eq!(refused.code(), ErrorCode::MESSAGE_NOT_DECODABLE);
    }

    #[test]
    fn an_event_this_build_has_no_name_for_is_refused_rather_than_dropped() {
        // The one place being lax would lose a whole run: an unknown terminal that decoded to
        // nothing would leave a reader waiting for an event that already came.
        let mut map: BTreeMap<&str, &str> = BTreeMap::new();
        map.insert("contract_version", CONTRACT_VERSION);
        map.insert("event", "invented");
        map.insert("id", "7");
        let payload = rmp_serde::to_vec_named(&map).expect("a map encodes");

        let refused = decode_event(&payload).expect_err("an unknown event is refused");
        assert_eq!(refused.code(), ErrorCode::MESSAGE_NOT_UNDERSTOOD);
    }

    #[test]
    fn a_field_this_build_does_not_know_is_ignored() {
        // The forward-compatible direction, and it is deliberate: an agent newer than this
        // application may add a field to an event, and the application carries on. The agent is
        // strict in the other direction, which is the correct asymmetry.
        let mut map: BTreeMap<&str, &str> = BTreeMap::new();
        map.insert("contract_version", CONTRACT_VERSION);
        map.insert("event", "completed");
        map.insert("id", "7");
        map.insert("invented_by_a_later_agent", "x");
        let payload = rmp_serde::to_vec_named(&map).expect("a map encodes");

        let event = decode_event(&payload).expect("an unknown field is ignored");
        assert_eq!(event.body, EventBody::Completed { id: "7".to_owned() });
    }

    #[test]
    fn a_failure_that_could_not_be_decoded_is_still_attributed_to_its_run() {
        let mut map: BTreeMap<&str, &str> = BTreeMap::new();
        map.insert("contract_version", "99");
        map.insert("event", "invented");
        map.insert("id", "7");
        let payload = rmp_serde::to_vec_named(&map).expect("a map encodes");

        assert_eq!(identifier_of(&payload), Some("7".to_owned()));
    }

    #[test]
    fn a_frame_that_never_had_an_identifier_is_not_made_to_yield_one() {
        assert_eq!(identifier_of(b"not messagepack at all"), None);

        let mut map: BTreeMap<&str, &str> = BTreeMap::new();
        map.insert("contract_version", CONTRACT_VERSION);
        map.insert("event", "ready");
        let payload = rmp_serde::to_vec_named(&map).expect("a map encodes");
        assert_eq!(identifier_of(&payload), None);
    }
}
