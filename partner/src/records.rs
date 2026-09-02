//! Where this program's records go.
//!
//! To standard error, in the line shape every Almena program writes, so that a partner's records
//! and a node's records read the same and can be put side by side. Standard output is kept for
//! what the program is asked to print — a link, an identifier, an outcome — so that a shell can
//! take that and nothing else.
//!
//! No file and no rotation: a partner is run by hand for one errand at a time, and whoever runs it
//! is looking at the terminal. A partner that ran as a service would want what the node has, and
//! would take it from `almena-log` the day it did.

use std::io::Write as _;
use std::sync::OnceLock;

use log::{Level, LevelFilter, Metadata, Record};

/// The destination every record of this program goes to.
struct Records;

impl log::Log for Records {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        metadata.level() <= Level::Info
    }

    fn log(&self, record: &Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let line = almena_log::line(record.level(), record.target(), record.args());
        // Best effort and silent: a logger that cannot write has nowhere to say so.
        let _ = writeln!(std::io::stderr(), "{line}");
    }

    fn flush(&self) {
        let _ = std::io::stderr().flush();
    }
}

/// Send this program's records to standard error.
///
/// Installing twice is a programming mistake rather than a condition, and the second call loses
/// silently, which is what makes this safe to call from a test that does not know whether another
/// test already did.
pub fn install() {
    static RECORDS: OnceLock<Records> = OnceLock::new();
    let records = RECORDS.get_or_init(|| Records);
    if log::set_logger(records).is_ok() {
        log::set_max_level(LevelFilter::Info);
    }
}
