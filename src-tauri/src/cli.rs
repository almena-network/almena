//! What this application answers when it is launched from a command line.
//!
//! Only `--help` and `--version`, and only on a computer: the arguments themselves are
//! declared in `tauri.conf.json` under `plugins.cli`, which is where the plugin reads them
//! from, so this module holds the one decision that cannot live in configuration — whether an
//! argument was a request to print something and stop, rather than to open a window.
//!
//! There is deliberately no `--node`. An address would be accepted, used for nothing and
//! refused by nothing, and this build has no client of the node API to honour one with —
//! `.claude/rules/transparency.md`, and `.claude/rules/ipv6-only.md` for the refusal that
//! would have to come with it.

use log::info;
use tauri::App;
use tauri_plugin_cli::CliExt;

/// Answers the arguments that are a question, and says whether the application should stop.
///
/// Returns `true` when something was printed and there is nothing left to do — the caller
/// exits rather than opening a window. Returns `false` for an ordinary launch, and for a
/// command line this build could not parse: a person who mistyped a flag is better served by
/// the application they asked for than by a refusal it cannot explain in their language
/// (`.claude/rules/user-facing-text.md`).
///
/// # The two arrive differently, and neither is obvious
///
/// This is the plugin's shape rather than a choice made here, and it was established by
/// running it rather than by reading about it:
///
/// - `--help` arrives as the key `help` carrying the text `clap` generated. There is something
///   to print and it is the value.
/// - `--version` arrives as the key `version` carrying **`null`, with zero occurrences**. The
///   presence of the key is the whole signal, so a test of its value — for a string, for
///   `true`, for anything — finds nothing and opens a window instead of answering.
///
/// An ordinary launch carries neither key: with no arguments declared of our own, the map is
/// empty, which is what makes presence safe to read as an answer.
pub fn answered(app: &App) -> bool {
    let Ok(matches) = app.cli().matches() else {
        info!("cli_arguments_not_understood");
        return false;
    };

    if let Some(text) = matches
        .args
        .get("help")
        .and_then(|data| data.value.as_str())
    {
        println!("{text}");
        return true;
    }

    if matches.args.contains_key("version") {
        // The crate's name and not `productName`, which is still the scaffold's `desktop` and
        // is 0001's open question. This is the name of the binary a person typed.
        println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
        return true;
    }

    false
}
