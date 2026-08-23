//! Where this program's records go, and what keeps them from filling a disk.
//!
//! The windowed application gets its destination, its rotation and its retention from
//! `tauri-plugin-log`. A program with no Tauri in it has to do that itself, and this is it —
//! the line shape and the two sizes still come from `almena-log`, so the two programs agree
//! about what a record looks like and about how much of one a disk ever holds.
//!
//! Nothing in here can report its own failure: a logger that cannot write has nowhere to say
//! so. Every write is therefore best-effort and silent, and losing records is preferred to
//! taking a running node down over one.

use std::fs::{File, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use log::{Level, LevelFilter, Metadata, Record};

/// The active file, and what is known about how full it is.
struct Active {
    /// The directory the files live in, kept so that rotation can name a sibling.
    directory: PathBuf,
    /// The program these files belong to, which is the prefix retention prunes by.
    program: String,
    /// The file being written to.
    file: File,
    /// How many bytes it holds, so that its size is not asked of the filesystem per record.
    written: u64,
}

impl Active {
    /// Writes one line, rotating first if this one would take the file past the limit.
    fn write(&mut self, line: &str) {
        if self.written >= almena_log::MAX_FILE_SIZE {
            self.rotate();
        }

        if self.file.write_all(line.as_bytes()).is_ok() && self.file.write_all(b"\n").is_ok() {
            self.written += line.len() as u64 + 1;
        }
    }

    /// Moves the active file aside under the moment it was rotated, and opens a new one.
    fn rotate(&mut self) {
        let rotated = self
            .directory
            .join(almena_log::rotated_file_name(&self.program, time_now()));
        let active = self
            .directory
            .join(almena_log::active_file_name(&self.program));

        if std::fs::rename(&active, rotated).is_err() {
            return;
        }

        if let Ok(file) = open(&active) {
            self.file = file;
            self.written = 0;
        }

        prune(&self.directory, &self.program);
    }
}

/// The destination every record of this program goes to.
struct Records {
    /// The file, when there is somewhere to keep one.
    active: Mutex<Option<Active>>,
    /// Whether records also go to the terminal, which is what `--quiet` is for.
    to_terminal: bool,
}

impl log::Log for Records {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        metadata.level() <= Level::Info
    }

    fn log(&self, record: &Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }

        let line = almena_log::line(record.level(), record.target(), record.args());

        if self.to_terminal {
            // Records go to standard error, so that standard output stays whatever the
            // program chooses to put there.
            let _ = writeln!(std::io::stderr(), "{line}");
        }

        if let Ok(mut active) = self.active.lock()
            && let Some(active) = active.as_mut()
        {
            active.write(&line);
        }
    }

    fn flush(&self) {
        if let Ok(mut active) = self.active.lock()
            && let Some(active) = active.as_mut()
        {
            let _ = active.file.flush();
        }
    }
}

/// Sends this program's records to `directory`, and to the terminal when asked.
///
/// `directory` is where log files are kept — `Paths::logs`. When it is absent, or cannot be
/// created, records still reach the terminal and nothing is written to disk: a node that
/// cannot write a log is still a node, and refusing to start over one would be the wrong
/// trade.
///
/// Returns the file records are being written to, so that a caller can say where they went,
/// and `None` when they are going nowhere but the terminal.
///
/// # Errors
///
/// Never returns one. Installing the logger twice is the only way this can fail and it is a
/// programming mistake rather than a condition, so it is silently the second call that loses.
pub fn install(program: &str, directory: Option<&Path>, to_terminal: bool) -> Option<PathBuf> {
    let active = directory.and_then(|directory| open_in(directory, program));
    let path = active.as_ref().map(|active| {
        active
            .directory
            .join(almena_log::active_file_name(&active.program))
    });

    // `log::set_logger` wants something that outlives the program, and the program is exactly
    // how long this lives. A `OnceLock` says that without leaking a box to say it, and it is
    // also what makes a second call to this function lose rather than install a second
    // destination.
    static RECORDS: OnceLock<Records> = OnceLock::new();

    let records = RECORDS.get_or_init(|| Records {
        active: Mutex::new(active),
        to_terminal,
    });

    if log::set_logger(records).is_ok() {
        log::set_max_level(LevelFilter::Info);
    }

    path
}

/// Opens this program's active file in `directory`, creating the directory if it is missing.
fn open_in(directory: &Path, program: &str) -> Option<Active> {
    std::fs::create_dir_all(directory).ok()?;

    let path = directory.join(almena_log::active_file_name(program));
    let file = open(&path).ok()?;
    let written = file.metadata().map(|data| data.len()).unwrap_or_default();

    Some(Active {
        directory: directory.to_path_buf(),
        program: program.to_owned(),
        file,
        written,
    })
}

/// Opens a log file for appending, creating it when it is not there.
fn open(path: &Path) -> std::io::Result<File> {
    OpenOptions::new().create(true).append(true).open(path)
}

/// Deletes the oldest rotated files until only [`almena_log::KEEP_FILES`] are left.
///
/// Prunes by prefix, which is why the active file must never carry a date — a dated active
/// name would leave every earlier day's files behind for ever.
///
/// **The prefix includes the separator**, and that is not a detail. A rotated file is
/// `<program>_<moment>.log`, so matching on `<program>` alone makes `almena` the prefix of
/// `almena-app`, and this program's retention deletes the windowed application's records.
/// `.agents/rules/logging.md` promises two processes never share a file; taking somebody
/// else's away is the same promise broken from the other end.
fn prune(directory: &Path, program: &str) {
    let prefix = format!("{program}_");
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };

    let mut rotated: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(&prefix))
        })
        .collect();

    // The name carries the moment, zero-padded and in one order, so sorting the names sorts
    // the moments. That is the whole reason the format is what it is.
    rotated.sort();

    let surplus = rotated.len().saturating_sub(almena_log::KEEP_FILES);
    for path in rotated.into_iter().take(surplus) {
        let _ = std::fs::remove_file(path);
    }
}

/// The moment, for naming a rotated file.
///
/// `almena-log` reads the clock itself for a record's timestamp and offers no way to ask it
/// for one, which is deliberate — see that crate. Rotation needs a moment too, and this is
/// the one place that is true.
fn time_now() -> time::OffsetDateTime {
    time::OffsetDateTime::now_utc()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{Active, open, prune};

    fn scratch(name: &str) -> PathBuf {
        let directory = std::env::temp_dir().join(format!("almena-records-{name}"));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).expect("a scratch directory");
        directory
    }

    #[test]
    fn retention_keeps_the_active_file_and_the_ten_newest() {
        let directory = scratch("retention");

        for day in 1..=15 {
            let name = format!("almena_2026-08-{day:02}_00-00-00.log");
            std::fs::write(directory.join(name), b"x").expect("a rotated file");
        }
        std::fs::write(directory.join("almena.log"), b"x").expect("the active file");

        prune(&directory, "almena");

        let left: Vec<String> = std::fs::read_dir(&directory)
            .expect("the directory")
            .filter_map(Result::ok)
            .filter_map(|entry| entry.file_name().into_string().ok())
            .collect();

        assert_eq!(left.len(), almena_log::KEEP_FILES + 1, "{left:?}");
        assert!(left.contains(&"almena.log".to_owned()), "{left:?}");
        assert!(
            left.contains(&"almena_2026-08-15_00-00-00.log".to_owned()),
            "the newest was pruned: {left:?}"
        );
        assert!(
            !left.contains(&"almena_2026-08-01_00-00-00.log".to_owned()),
            "the oldest survived: {left:?}"
        );
    }

    #[test]
    fn another_programs_files_are_never_pruned() {
        let directory = scratch("prefix");

        for day in 1..=15 {
            let name = format!("almena-app_2026-08-{day:02}_00-00-00.log");
            std::fs::write(directory.join(name), b"x").expect("a rotated file");
        }

        prune(&directory, "almena");

        let left = std::fs::read_dir(&directory)
            .expect("the directory")
            .filter_map(Result::ok)
            .count();
        assert_eq!(left, 15, "retention reached past its own program");
    }

    #[test]
    fn rotation_moves_the_active_file_aside_and_opens_a_new_one() {
        let directory = scratch("rotation");
        let path = directory.join("almena.log");
        let file = open(&path).expect("the active file");

        let mut active = Active {
            directory: directory.clone(),
            program: "almena".to_owned(),
            file,
            written: 0,
        };

        active.write("first");
        active.rotate();
        active.write("second");

        let active_text = std::fs::read_to_string(&path).expect("the new active file");
        assert_eq!(active_text.trim(), "second");

        let rotated: Vec<String> = std::fs::read_dir(&directory)
            .expect("the directory")
            .filter_map(Result::ok)
            .filter_map(|entry| entry.file_name().into_string().ok())
            .filter(|name| name != "almena.log")
            .collect();
        assert_eq!(rotated.len(), 1, "{rotated:?}");

        let moved = std::fs::read_to_string(directory.join(&rotated[0])).expect("the rotated file");
        assert_eq!(moved.trim(), "first");
    }
}
