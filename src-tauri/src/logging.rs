//! Logging: where this program's records go, and how loud it is.
//!
//! The **format** is not decided here, and neither are the sizes. Both are [`almena_log`]'s,
//! which the terminal interface will call too, so that the programs built in this repository
//! write the same record and are bounded by the same numbers despite installing logging
//! through entirely different machinery — this one through `tauri-plugin-log`, and a program
//! with no Tauri in it through whatever suits it.
//!
//! What is decided here is this program's own part: its name, its destinations and its level.
//! Two programs never share a log file, which is why the name below is a constant of this
//! module and not a value passed in.

use tauri::Wry;
use tauri::plugin::TauriPlugin;
use tauri_plugin_log::{Builder, RotationStrategy, Target, TargetKind, log::LevelFilter};

/// The name this program's log files carry.
///
/// `mainBinaryName` from `tauri.conf.json`, so that the file is named after the thing that
/// wrote it. It must not carry the date: the plugin prunes only files whose name begins with
/// this one, and a name that changed every day would leave every previous day's files behind
/// for ever.
pub const PROGRAM: &str = "almena-app";

/// The configured log plugin.
///
/// One line per record, the same on every platform and in every destination:
///
/// ```text
/// 2026-08-12T14:51:03.123Z INFO  almena_app_lib::window window_shown
/// ```
///
/// Timestamps are UTC, which [`almena_log`] enforces by taking them itself rather than
/// accepting one: Almena is a network of machines in different places, and logs from two of
/// them are only comparable if they share a clock.
///
/// Registered on every platform, unlike the two beside it. A phone writes records too, and
/// deleting the whole log directory while the application is closed costs nothing but the
/// history.
pub fn plugin() -> TauriPlugin<Wry> {
    Builder::new()
        .targets([
            Target::new(TargetKind::Stdout),
            Target::new(TargetKind::LogDir {
                file_name: Some(PROGRAM.into()),
            }),
        ])
        // Debug while developing, Info in a release build. Anything noisier belongs behind
        // `level_for` for the module that needs it, not in the global level.
        .level(if cfg!(debug_assertions) {
            LevelFilter::Debug
        } else {
            LevelFilter::Info
        })
        .rotation_strategy(RotationStrategy::KeepSome(almena_log::KEEP_FILES))
        .max_file_size(u128::from(almena_log::MAX_FILE_SIZE))
        .format(|out, message, record| {
            out.finish(format_args!(
                "{}",
                almena_log::line(record.level(), record.target(), message)
            ));
        })
        .build()
}
