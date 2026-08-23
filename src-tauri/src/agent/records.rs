//! The agent's own records, forwarded into this program's log.
//!
//! The agent writes to its stderr in the same line shape every Almena program writes — that is
//! `.agents/rules/logging.md` applied to a program in another language, and `almena-log` is
//! the crate that defines it. What it does **not** do is open a file: it writes to a pipe, and
//! this side writes the record. So one file holds two programs' records and nothing competes
//! for a file handle, which is the thing that rule's *two processes never share a file* exists
//! to prevent.
//!
//! # Why the line is taken apart rather than passed on
//!
//! Handing the whole line to `log::info!` would stamp it twice: `almena_log::line` prepends its
//! own timestamp, level and target to whatever it is given, so a forwarded record would arrive
//! with two of each. So the fields are read off and re-emitted, and two things change on the
//! way:
//!
//! - **The agent's timestamp is dropped and this program's clock stamps the record.** Not
//!   carelessness: `almena-log` takes the clock itself, deliberately, so that no caller can
//!   pass one in. Both clocks are UTC on one machine and the difference between them is the
//!   length of a pipe.
//! - **The target is prefixed**, so that one file can be read as two programs and a reader can
//!   `grep` for either half.
//!
//! # What is never forwarded
//!
//! Nothing that crosses the wire. Tokens, the text of a turn and a tool's arguments are the
//! content of a person's conversation, and `.agents/rules/logging.md` rules out logging that in
//! as many words. This module only ever sees the agent's stderr, which by the agent's own
//! design carries none of it — but the supervisor is held to the same line, and says so where
//! it writes its own records.

use log::{Level, log, warn};

/// What every forwarded record's target begins with.
const FROM: &str = "almena_agent";

/// How much of a line that could not be read is repeated back.
const KEPT: usize = 120;

/// Forwards one line of the agent's stderr into this program's log.
///
/// A line that is not a record of the shared shape is reported once, truncated, rather than
/// being passed through — a library writing its own format to stderr must not be able to forge
/// a level or a target in this program's log.
pub fn forward(line: &str) {
    let trimmed = line.trim_end();
    if trimmed.is_empty() {
        return;
    }

    match read(trimmed) {
        Some((level, target, message)) => log!(target: &target, level, "{message}"),
        None => {
            let kept: String = trimmed.chars().take(KEPT).collect();
            warn!("agent_record_not_understood line={kept:?}");
        }
    }
}

/// The level, the target and the message of one record, or nothing when it is not one.
///
/// The shape is `<timestamp> <LEVEL> <name> <message>`, which is `.agents/rules/logging.md`'s
/// and is what the agent's `records.py` writes. The timestamp is read only far enough to know
/// it was there; its value is deliberately discarded.
fn read(line: &str) -> Option<(Level, String, String)> {
    // Split one field at a time, trimming before each: the level is padded to five columns, so
    // what separates it from the name is a *run* of spaces rather than one. Splitting on every
    // whitespace character instead would hand back the empty string between them, which is the
    // bug these tests were written to catch and did.
    let (stamp, rest) = line.trim_start().split_once(char::is_whitespace)?;
    if !stamp.ends_with('Z') {
        return None;
    }

    let (level, rest) = rest.trim_start().split_once(char::is_whitespace)?;
    let level = level_of(level)?;

    let (name, message) = rest.trim_start().split_once(char::is_whitespace)?;

    Some((level, target_of(name), message.trim_start().to_owned()))
}

/// The target a record from `name` is written under.
///
/// Python separates a logger's parts with dots and Rust with colons, so the name is rewritten
/// into the shape a reader of this log already knows. The agent's own modules arrive already
/// carrying the package name, so it is taken off rather than written twice — and a record from
/// a library the agent happens to use, which carries no package name at all, still ends up
/// under one. Either way the target says which program wrote it, which is the whole job.
fn target_of(name: &str) -> String {
    let within = name.strip_prefix("almena_agent.").unwrap_or(name);
    format!("{FROM}::{}", within.replace('.', "::"))
}

/// One of the five levels, or nothing where the field was something else.
///
/// Matched by hand rather than through `FromStr`: `log`'s own parser accepts names this format
/// never writes, and a record whose level this program cannot read should be reported rather
/// than guessed at.
fn level_of(field: &str) -> Option<Level> {
    match field {
        "ERROR" => Some(Level::Error),
        "WARN" => Some(Level::Warn),
        "INFO" => Some(Level::Info),
        "DEBUG" => Some(Level::Debug),
        "TRACE" => Some(Level::Trace),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use log::Level;

    use super::read;

    #[test]
    fn a_record_is_read_into_its_level_its_target_and_its_message() {
        let (level, target, message) =
            read("2026-08-23T19:39:01.607Z INFO  almena_agent.stdio agent_ready model=gemma")
                .expect("a record of the shared shape is read");

        assert_eq!(level, Level::Info);
        assert_eq!(target, "almena_agent::stdio");
        assert_eq!(message, "agent_ready model=gemma");
    }

    #[test]
    fn the_target_says_which_program_wrote_it_and_never_says_it_twice() {
        // One file holds both programs' records, so a reader has to be able to tell them
        // apart — and `almena_agent::almena_agent::session` would be a reader's problem too.
        let (_, mine, _) = read("2026-08-23T19:39:01.607Z ERROR almena_agent.session x")
            .expect("a record is read");
        assert_eq!(mine, "almena_agent::session");

        // A library the agent uses writes no package name of its own, and still lands under
        // one, so nothing in this file is unattributable.
        let (_, theirs, _) =
            read("2026-08-23T19:39:01.607Z WARN  httpx x").expect("a record is read");
        assert_eq!(theirs, "almena_agent::httpx");
    }

    #[test]
    fn a_level_of_every_width_is_read_because_the_field_is_padded() {
        for (written, meant) in [
            ("ERROR", Level::Error),
            ("WARN ", Level::Warn),
            ("INFO ", Level::Info),
            ("DEBUG", Level::Debug),
            ("TRACE", Level::Trace),
        ] {
            let line = format!("2026-08-23T19:39:01.607Z {written} a.b said");
            let (level, _, message) = read(&line).expect("every level is read");
            assert_eq!(level, meant);
            assert_eq!(message, "said", "the padding is not part of the message");
        }
    }

    #[test]
    fn a_line_that_is_not_a_record_is_refused_rather_than_passed_through() {
        // A library writing its own format to stderr must not be able to forge a level or a
        // target in this program's log.
        for line in [
            "Traceback (most recent call last):",
            "INFO something without a timestamp",
            "2026-08-23T19:39:01.607Z NOTICE a.b said",
            "2026-08-23T19:39:01.607Z INFO",
            "",
        ] {
            assert!(read(line).is_none(), "{line:?}");
        }
    }

    #[test]
    fn a_message_carrying_spaces_survives_whole() {
        let (_, _, message) =
            read("2026-08-23T19:39:01.607Z WARN  a.b run_failed id=3 code=resource_unknown")
                .expect("a record is read");

        assert_eq!(message, "run_failed id=3 code=resource_unknown");
    }
}
