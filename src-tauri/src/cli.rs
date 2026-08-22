//! What this application answers when it is launched from a command line.
//!
//! Only on a computer, and only three things: `--help`, `--version`, and `--hidden`. The
//! arguments themselves are declared in `tauri.conf.json` under `plugins.cli`, which is where
//! the plugin reads them from, so this module holds the two decisions that cannot live in
//! configuration — whether an argument was a request to print something and stop, and whether
//! this launch was meant to put a window on the screen at all.
//!
//! There is deliberately no argument naming a peer or a network. This build joins no network,
//! so an address would be accepted, used for nothing and refused by nothing. An argument that
//! does nothing is a lie about the application, and accepting an address means refusing every
//! IPv4 one — Almena is IPv6-only — so the flag arrives with the code that can honour both
//! halves.

use log::info;
use tauri::App;
use tauri_plugin_cli::CliExt;

/// Answers the arguments that are a question, and says whether the application should stop.
///
/// Returns `true` when something was printed and there is nothing left to do — the caller
/// exits rather than opening a window. Returns `false` for an ordinary launch, and for a
/// command line this build could not parse: a person who mistyped a flag is better served by
/// the application they asked for than by a refusal it cannot explain in their language
/// (`.agents/rules/user-facing-text.md`).
///
/// # The two arrive differently, and neither is obvious
///
/// This is the plugin's shape rather than a choice made here, and it was established by
/// reading its parser rather than by guessing:
///
/// - `--help` arrives as the key `help` carrying the text `clap` generated. There is something
///   to print and it is the value.
/// - `--version` arrives as the key `version` carrying **`null`, with zero occurrences**. The
///   presence of the key is the whole signal, so a test of its value — for a string, for
///   `true`, for anything — finds nothing and opens a window instead of answering.
///
/// **Reading them by presence stays correct now that this application declares an argument of
/// its own**, and the reason is worth writing down because it is not obvious. Both come out of
/// `clap` as an *error* — `DisplayHelp` and `DisplayVersion` — which the plugin turns into a
/// map built from nothing, holding that one key. Declared arguments are only mapped on the
/// other branch, the one where parsing succeeded. The two can never share a map, so nothing
/// of ours can ever be mistaken for one of them.
///
/// [`starts_hidden`] is on that other branch and therefore reads a value rather than a
/// presence: on an ordinary launch every declared argument is in the map whether it was typed
/// or not, a flag that was not typed sitting there as `false`.
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

/// Whether this launch was asked to start with no window.
///
/// `--hidden` is what the operating system passes when it starts Almena at login, because the
/// plugin that registered it there registered it with that argument. An application that puts
/// a window in front of somebody who was logging in, rather than waiting in the tray until it
/// is asked for, is an application they switch off within the week.
///
/// It is read by value and not by presence — see the section above on why the other two are
/// the other way round. A command line this build could not parse starts an ordinary visible
/// launch, for the same reason `answered` returns `false` in that case: the window a person
/// asked for serves them better than a refusal nobody can explain in their language.
pub fn starts_hidden(app: &App) -> bool {
    let Ok(matches) = app.cli().matches() else {
        return false;
    };

    matches
        .args
        .get("hidden")
        .and_then(|data| data.value.as_bool())
        .unwrap_or(false)
}
