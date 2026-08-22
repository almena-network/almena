//! The log format every Almena program writes, and the sizes it is bounded by.
//!
//! Two people reading the logs of two different Almena programs must not have to learn two
//! formats. That is easy to agree and hard to keep, because the programs built here install
//! their logging through different machinery — the windowed application through
//! `tauri-plugin-log`, and anything else through whatever suits it, since a Tauri plugin is no
//! use to a program with no Tauri in it.
//!
//! So the machinery differs and **the format is defined here, once**. Every program calls
//! [`line`] and none writes a format string of its own. A second definition would agree with
//! the first for about a month.
//!
//! # The format
//!
//! ```text
//! <timestamp> <LEVEL> <target> <message>
//! ```
//!
//! ```text
//! 2026-08-12T14:51:03.123Z INFO  almena_app_lib::window window_restored
//! ```
//!
//! The timestamp is RFC 3339 with milliseconds, **always UTC**, always ending in `Z`. Almena
//! is a network of machines in different time zones and two logs are only comparable if they
//! share a clock; local time is for an interface, never for a record. The level is padded to
//! five columns so the fields line up when read by eye.
//!
//! A message is an identifier followed by `key=value` pairs, not a sentence with values in it:
//! `peer_handshake_failed peer=2f80` can be grepped a year from now and
//! "Could not shake hands with peer 2f80" cannot. This module cannot enforce that and a
//! reviewer can.
//!
//! # It takes the time rather than accepting it
//!
//! [`line`] reads the clock itself. A caller that passed one in could pass a local one, and
//! the whole point of the paragraph above is that nobody gets to.
//!
//! See `.agents/rules/logging.md`, which this crate is the enforceable half of.

use std::fmt::Arguments;

use log::Level;
use time::OffsetDateTime;
use time::format_description::BorrowedFormatItem;
use time::macros::format_description;

/// How the timestamp of every record is written.
const TIMESTAMP: &[BorrowedFormatItem<'_>] =
    format_description!("[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:3]Z");

/// A log file is rotated once it reaches this size.
pub const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024;

/// How many rotated files are kept besides the active one, oldest deleted first.
///
/// Ten, which bounds a program at about 110 MiB on disk. A repository that ships three of them
/// is bounded at three times that, and every one of those files is the operator's to delete at
/// any moment.
pub const KEEP_FILES: usize = 10;

/// The name of a program's active log file.
///
/// **Never carries the date.** Retention prunes only files whose name begins with the active
/// one, so a name that changed every day would leave every previous day's files behind for
/// ever.
///
/// # Examples
///
/// ```
/// assert_eq!(almena_log::active_file_name("almena-app"), "almena-app.log");
/// ```
#[must_use]
pub fn active_file_name(program: &str) -> String {
    format!("{program}.log")
}

/// The name a file takes when it is rotated: the program, and the moment it was rotated, UTC.
///
/// # Examples
///
/// ```
/// # use time::macros::datetime;
/// let moment = datetime!(2026-08-12 14:51:03 UTC);
/// assert_eq!(
///     almena_log::rotated_file_name("almena-app", moment),
///     "almena-app_2026-08-12_14-51-03.log"
/// );
/// ```
#[must_use]
pub fn rotated_file_name(program: &str, moment: OffsetDateTime) -> String {
    let stamp = format_description!("[year]-[month]-[day]_[hour]-[minute]-[second]");

    moment.format(stamp).map_or_else(
        // A clock the formatter cannot render is not worth losing a file over: the rotated
        // file still needs a name, and one without a moment in it is better than none.
        |_| format!("{program}_unknown.log"),
        |written| format!("{program}_{written}.log"),
    )
}

/// One record, formatted.
///
/// # Examples
///
/// ```
/// use log::Level;
///
/// let line = almena_log::line(Level::Info, "almena_app_lib::window", &format_args!("shown"));
/// assert!(line.ends_with(" INFO  almena_app_lib::window shown"));
/// ```
#[must_use]
pub fn line(level: Level, target: &str, message: &Arguments<'_>) -> String {
    format!("{} {:<5} {target} {message}", now(), level)
}

/// The moment, as every record writes it: RFC 3339, milliseconds, UTC, `Z`.
#[must_use]
pub fn now() -> String {
    OffsetDateTime::now_utc()
        .format(TIMESTAMP)
        // A clock that cannot be rendered is not a reason to lose the record. The shape stays
        // fixed-width so that a reader's eye and a `cut` both still find the fields.
        .unwrap_or_else(|_| "0000-00-00T00:00:00.000Z".to_owned())
}

#[cfg(test)]
mod tests {
    use log::Level;
    use time::macros::datetime;

    use super::{active_file_name, line, now, rotated_file_name};

    #[test]
    fn the_moment_is_utc_and_says_so() {
        let moment = now();
        assert!(moment.ends_with('Z'), "{moment}");
        assert_eq!(moment.len(), "2026-08-12T14:51:03.123Z".len(), "{moment}");
    }

    #[test]
    fn a_record_is_four_fields_in_one_line() {
        let record = line(
            Level::Warn,
            "almena_app_lib::window",
            &format_args!("a b=c"),
        );
        assert!(!record.contains('\n'), "{record}");

        // Split on runs of space rather than on single ones: the level is padded to five
        // columns, so how many spaces follow it depends on which level it was.
        let mut fields = record.split_whitespace();
        assert!(
            fields.next().is_some_and(|stamp| stamp.ends_with('Z')),
            "{record}"
        );
        assert_eq!(fields.next(), Some("WARN"), "{record}");
        assert_eq!(fields.next(), Some("almena_app_lib::window"), "{record}");
        assert_eq!(fields.next(), Some("a"), "{record}");
        assert_eq!(fields.next(), Some("b=c"), "{record}");
    }

    #[test]
    fn the_level_is_padded_so_the_columns_line_up() {
        let short = line(Level::Info, "t", &format_args!("m"));
        let long = line(Level::Error, "t", &format_args!("m"));
        let column = |record: &str| record.find(" t ").expect("the target follows the level");
        assert_eq!(column(&short), column(&long));
    }

    #[test]
    fn the_active_file_never_carries_a_date() {
        // Retention prunes by prefix, so a dated active name would strand every earlier day's
        // files for ever. This is the test that would catch somebody making it friendlier.
        let name = active_file_name("almena-app");
        assert_eq!(name, "almena-app.log");
        assert!(!name.contains("20"), "{name}");
    }

    #[test]
    fn a_rotated_file_carries_the_moment_it_was_rotated() {
        let name = rotated_file_name("almena-app", datetime!(2026-08-12 14:51:03 UTC));
        assert_eq!(name, "almena-app_2026-08-12_14-51-03.log");
        assert!(name.starts_with("almena-app"), "retention prunes by prefix");
    }
}
