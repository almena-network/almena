//! The windowed application: what is registered, and in which order.
//!
//! Every module beside this one owns a concern of its own. This one owns none of them — it
//! exists so that the order they are wired in is written down in a single place, because
//! several of them only work when registered before or after another, and because that order
//! is where the desktop and the mobile shapes of this application differ.

// `almena` is one codebase in two shapes. On a computer the application remembers its
// geometry, refuses to run twice and answers a command line; on a phone the operating system
// owns all three, so the modules that serve them are marked `#[cfg(desktop)]` and are not in
// the mobile binary at all.
#[cfg(desktop)]
mod cli;
mod logging;
#[cfg(desktop)]
mod window;

use log::info;

#[cfg(desktop)]
use tauri_plugin_window_state::StateFlags;

/// What the window remembers between runs.
///
/// Everything the plugin tracks: size, position, maximised, decorations, fullscreen and
/// visibility. `client` withholds `VISIBLE` because closing its window only hides it to a
/// tray, so a session ended that way would be restored with no window on screen. This
/// application has no tray yet and closing it quits, so a run always ends visible and there is
/// nothing to withhold. **That changes in the same commit the tray arrives.**
#[cfg(desktop)]
fn window_state_flags() -> StateFlags {
    StateFlags::all()
}

/// Greets whoever is named. The scaffold's command, kept until there is a real one.
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {name}! You've been greeted from Rust!")
}

/// Builds the application and runs it until its last window closes.
///
/// # Panics
///
/// Panics when Tauri cannot build a context — a missing `tauri.conf.json`, or an asset the
/// build embedded and the binary cannot read. There is nothing to fall back to: an
/// application that cannot construct itself has no window to report the failure in, so it
/// fails loudly here rather than starting into an empty screen.
pub fn run() {
    let builder = tauri::Builder::default();

    // Single instance goes first: the plugin's own documentation requires it to be the first
    // one registered. A second launch does not start a second application — it brings the
    // running one back to the person who asked for it.
    //
    // There is no mobile equivalent to add here, and none is needed: iOS and Android already
    // guarantee a single running instance.
    #[cfg(desktop)]
    let builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
        info!("second_launch_folded_into_running_instance");
        window::show_main(app);
    }));

    // Logging comes next so that everything registered below it can write a record. Not a
    // desktop concern: a phone writes records too, and the plugin supports Android and iOS.
    let builder = builder
        .plugin(logging::plugin())
        .plugin(tauri_plugin_opener::init());

    #[cfg(desktop)]
    let builder = builder.plugin(
        tauri_plugin_window_state::Builder::default()
            .with_state_flags(window_state_flags())
            .build(),
    );

    // The command line last of the three, and after logging: what it answers is decided with
    // the application already built, because the plugin reads the matches through the handle.
    #[cfg(desktop)]
    let builder = builder.plugin(tauri_plugin_cli::init());

    let app = builder
        .invoke_handler(tauri::generate_handler![greet])
        .build(tauri::generate_context!());

    match app {
        Ok(app) => {
            // `--help` and `--version` are a question, not a launch. Answered before the
            // window exists, so that a person who asked one never sees one open.
            #[cfg(desktop)]
            if cli::answered(&app) {
                return;
            }

            info!("application_started");
            app.run(|_handle, _event| {});
        }
        Err(error) => panic!("the application could not be built: {error}"),
    }
}

/// Where a phone starts the application.
///
/// `tauri::mobile_entry_point` emits a public function of its own and gives it no doc comment,
/// which a crate that denies `missing_docs` cannot compile — the failure only appears on a
/// mobile target, so a desktop build passes and `task deploy:ios` is where it turns up.
///
/// The allow is on this module rather than on the function because the macro emits a **new**
/// item: an attribute on what it consumed does not cover what it produces. A module is the
/// smallest scope that does cover it, and this one holds one function, so nothing else can
/// quietly go undocumented under the same allow.
#[cfg(mobile)]
#[allow(missing_docs)]
mod entry {
    #[tauri::mobile_entry_point]
    fn start() {
        super::run();
    }
}
