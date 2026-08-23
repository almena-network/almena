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
#[cfg(desktop)]
mod geometry;
mod logging;
// The one public module of this crate, and public on purpose. Every other module here is
// wiring that only `run` below has any business calling; `notification` is what this crate
// offers to whatever comes to have something to announce, including code that runs when no
// window exists (`.agents/rules/modularity-and-reuse.md`).
pub mod notification;
#[cfg(desktop)]
mod open_at_login;
// What a person chose about how the interface looks and which language it speaks. On every
// platform: a phone has a palette and a language like anything else.
mod preferences;
#[cfg(desktop)]
mod tray;
#[cfg(desktop)]
mod window;

use log::info;

#[cfg(desktop)]
use tauri::Manager;
#[cfg(desktop)]
use tauri_plugin_window_state::StateFlags;

/// What the window remembers between runs.
///
/// Everything the plugin tracks — size, position, maximised, decorations, fullscreen — except
/// **visibility**, and that exception is the tray's doing. This application no longer ends when
/// its window is closed: it hides, and waits to be asked for. A session that ended hidden would
/// therefore be restored hidden, and an application that starts into nothing at all is one
/// nobody can tell from a broken one.
///
/// Withholding `VISIBLE` is what keeps a launch a launch. Whether a window appears is decided
/// by `cli::starts_hidden` and by nothing that was written to disk last time.
#[cfg(desktop)]
fn window_state_flags() -> StateFlags {
    StateFlags::all().difference(StateFlags::VISIBLE)
}

/// Whether this is a development build rather than one somebody was given.
///
/// `debug_assertions` and not `tauri::is_dev`: the second is true only while the interface is
/// being served by a dev server, which would leave `task build:debug` — a bundle with the
/// assertions still in it — indistinguishable from a release. What the interface says with
/// this is *this is not the application anybody is meant to be running*, and the debug profile
/// is exactly that question.
///
/// On every platform, unlike the two below it. `task dev:ios` is a development run like any
/// other, and a phone showing no marker would be the one place the marker could be missed.
#[tauri::command]
fn is_development() -> bool {
    cfg!(debug_assertions)
}

/// Whether this build is the one that runs on a computer.
///
/// The interface needs it for one thing: starting with the system and the tray belong to a
/// computer, and a screen must not offer a control for something the platform does not have.
/// It is asked of this side rather than worked out from the user agent, because the binary
/// knows and a user agent is a guess.
#[tauri::command]
fn is_desktop() -> bool {
    cfg!(desktop)
}

/// Registers everything only a computer has, and returns the builder with it on.
///
/// Apart from [`assemble`] because it is the half that does not exist on a phone, not because
/// it is a different kind of work: the window it remembers between runs, the command line it
/// answers, who may open it at login, and the close button that no longer ends it.
///
/// Single instance is not here, and cannot be: its own documentation requires it to be the
/// first plugin registered of all, which is before this function is reached.
#[cfg(desktop)]
fn assemble_desktop(builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    let builder = builder.plugin(
        tauri_plugin_window_state::Builder::default()
            .with_state_flags(window_state_flags())
            .build(),
    );

    // The command line last of the plugins that answer one, and after logging: what it answers
    // is decided with the application already built, because the plugin reads the matches
    // through the handle.
    let builder = builder.plugin(tauri_plugin_cli::init());

    // Whether the operating system opens this application when somebody logs in. The argument
    // is not decoration: it is written into the entry, so the launch that comes out of it is one
    // `cli::starts_hidden` recognises and nobody gets a window in their face at login.
    //
    // **Windows and Linux only.** Each keeps one register for this and the plugin writes to it.
    // macOS keeps two — one for opening at login and one for running in the background — and
    // this plugin can only write the second, which is the wrong one. `open_at_login` is where
    // that is explained and where macOS is served instead.
    //
    // A phone is absent from both: its operating system owns when an application runs and
    // offers nothing to switch.
    #[cfg(any(windows, target_os = "linux"))]
    let builder = builder.plugin(tauri_plugin_autostart::init(
        tauri_plugin_autostart::MacosLauncher::LaunchAgent,
        Some(vec!["--hidden"]),
    ));

    // Two plugins that nothing calls yet, and both are here on purpose rather than by
    // accident. `dialog` is what a native question, warning or file picker will come from the
    // day something has one to ask. `updater` is what lets an application that is a file
    // somebody downloaded replace itself — and it is registered **inert**: nothing in this
    // application asks it anything, and what it may and may not do when something does is
    // `.agents/rules/updating.md`, not a builder argument.
    let builder = builder
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build());

    let builder = builder.on_window_event(on_window_event);

    // Where the size a person gave the window is held between the resize that produced it and
    // the exit that writes it down.
    let builder = builder.manage(geometry::Seen::default());

    // A launch the system started at login puts nothing on the screen. The window is still
    // built and the interface still loads — the tray has to be named from there — it is simply
    // never shown.
    //
    // This is in `setup` and not beside the command-line answer in `run`, and the reason was
    // found by running it: the window declared in `tauri.conf.json` does not exist yet when
    // `build` returns. Asking for it there finds nothing to hide and says so in the log.
    builder.setup(|app| {
        // After the plugin has restored the geometry and before anybody has looked at it: by
        // now the window is on the display it will open on, which is what makes the comparison
        // inside this call a comparison between two sizes on the same screen.
        if let Some(window) = window::main(app.handle()) {
            geometry::restore(&window);
        }

        if cli::starts_hidden(app) {
            info!("started_hidden");
            window::hide_main(app.handle());
        }

        Ok(())
    })
}

/// What happens to the window when the platform says something about it.
///
/// Two things do, and they are unrelated except in arriving here. The close button stops ending
/// the application and starts putting it away — but only where there is a tray to find it in
/// again; if the tray failed to build, a close is a close and the application ends the way it
/// always did, rather than vanishing to somewhere this screen has no route back to. And every
/// resize is the size a person is choosing, which is what `geometry` is there to give back.
#[cfg(desktop)]
fn on_window_event(window: &tauri::Window, event: &tauri::WindowEvent) {
    match event {
        tauri::WindowEvent::CloseRequested { api, .. } => {
            if tray::installed(window.app_handle()) {
                api.prevent_close();
                window::hide_main(window.app_handle());
            }
        }
        tauri::WindowEvent::Resized(size) => geometry::note(window, *size),
        _ => {}
    }
}

/// Registers everything this application is made of, in the order it has to happen.
///
/// Separate from [`run`] because they are two jobs: this one decides what the application *is*
/// and nothing here can fail, while `run` builds it, answers whatever the command line asked
/// and hands control to the platform. Reading either without the other in the way is the point.
fn assemble() -> tauri::Builder<tauri::Wry> {
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
    //
    // Notifications are registered here for the same reason and with the same reach — all five
    // platforms, so the frontend's way to them and `notification`'s are available wherever this
    // application runs. Order does not matter to this one; it sits with the others that are not
    // a desktop concern rather than among the three that are.
    let builder = builder
        .plugin(logging::plugin())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init());

    #[cfg(desktop)]
    let builder = assemble_desktop(builder);

    // What the interface may call. The desktop build answers one more than the mobile one: the
    // tray is built from that side because its menu is text a person reads, and the catalogs
    // are there and not here.
    #[cfg(desktop)]
    let builder = builder.invoke_handler(tauri::generate_handler![
        is_desktop,
        is_development,
        tray::install_tray,
        open_at_login::opens_at_login,
        open_at_login::set_opens_at_login,
        preferences::preferences,
        preferences::set_preferences
    ]);
    #[cfg(mobile)]
    let builder = builder.invoke_handler(tauri::generate_handler![
        is_desktop,
        is_development,
        preferences::preferences,
        preferences::set_preferences
    ]);

    builder
}

/// Builds the application and runs it until something ends it.
///
/// Not "until its last window closes" any more: with a tray on the bar the window closing is
/// the window going away, and the application ends when the tray's own entry says so.
///
/// # Panics
///
/// Panics when Tauri cannot build a context — a missing `tauri.conf.json`, or an asset the
/// build embedded and the binary cannot read. There is nothing to fall back to: an
/// application that cannot construct itself has no window to report the failure in, so it
/// fails loudly here rather than starting into an empty screen.
pub fn run() {
    let app = assemble().build(tauri::generate_context!());

    match app {
        Ok(app) => {
            // `--help` and `--version` are a question, not a launch. Answered before the
            // window exists, so that a person who asked one never sees one open.
            #[cfg(desktop)]
            if cli::answered(&app) {
                return;
            }

            info!("application_started");
            app.run(|handle, event| {
                // The size the window ended up at is written down here and nowhere else: it is
                // the one moment at which nothing more can change it.
                //
                // macOS, and only macOS, has a second thing to answer: clicking the Dock icon
                // of a running application with no window on screen is a request for it back.
                // The other two desktops have no Dock and come back through the launcher
                // instead, which the single-instance plugin turns into the very same call.
                #[cfg(desktop)]
                match event {
                    tauri::RunEvent::Exit => geometry::save(handle),
                    #[cfg(target_os = "macos")]
                    tauri::RunEvent::Reopen { .. } => window::show_main(handle),
                    _ => {}
                }

                #[cfg(not(desktop))]
                let _ = (handle, event);
            });
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
