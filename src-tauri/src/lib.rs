//! The windowed application: what is registered, and in which order.
//!
//! Every module beside this one owns a concern of its own. This one owns none of them — it
//! exists so that the order they are wired in is written down in a single place, because
//! several of them only work when registered before or after another.
//!
//! There is one shape of this application and there used to be two. It ran on phones as well
//! as on computers, and the modules a phone had no use for were marked `#[cfg(desktop)]`; the
//! phone application is a repository of its own now and this one is built for Windows, macOS
//! and Linux alone, so a condition that can never be false is not written.
//! What remains conditional below is the genuine difference **between those three** — which
//! register an operating system keeps for opening an application at login, and the Dock.

mod agent;
mod geometry;
// Public so that its one question can be asked in a doctest. Nothing outside this crate calls
// it.
pub mod launch;
mod logging;
// The one public module of this crate, and public on purpose. Every other module here is
// wiring that only `run` below has any business calling; `notification` is what this crate
// offers to whatever comes to have something to announce, including code that runs when no
// window exists — and what leaves the crate is the only thing `pub` is for.
mod node;
pub mod notification;
mod open_at_login;
// What a person chose about how the interface looks and which language it speaks.
mod preferences;
mod tray;
mod window;

use log::info;

use tauri::Manager;
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
/// by `launch::starts_hidden` and by nothing that was written to disk last time.
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
#[tauri::command]
fn is_development() -> bool {
    cfg!(debug_assertions)
}

/// Registers the window, the login entry and the two inert plugins, and returns the builder.
///
/// Apart from [`assemble`] to keep that function readable, not because it is a different kind
/// of work: the window this remembers between runs, who may open it at login, and the close
/// button that no longer ends it.
///
/// Single instance is not here, and cannot be: its own documentation requires it to be the
/// first plugin registered of all, which is before this function is reached.
fn assemble_desktop(builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    let builder = builder.plugin(
        tauri_plugin_window_state::Builder::default()
            .with_state_flags(window_state_flags())
            .build(),
    );

    // Whether the operating system opens this application when somebody logs in. The argument
    // is not decoration: it is written into the entry, so the launch that comes out of it is one
    // `launch::starts_hidden` recognises and nobody gets a window in their face at login.
    //
    // **Windows and Linux only.** Each keeps one register for this and the plugin writes to it.
    // macOS keeps two — one for opening at login and one for running in the background — and
    // this plugin can only write the second, which is the wrong one. `open_at_login` is where
    // that is explained and where macOS is served instead.
    #[cfg(any(windows, target_os = "linux"))]
    let builder = builder.plugin(tauri_plugin_autostart::init(
        tauri_plugin_autostart::MacosLauncher::LaunchAgent,
        Some(vec!["--hidden"]),
    ));

    // Two plugins that nothing calls yet, and both are here on purpose rather than by
    // accident. `dialog` is what a native question, warning or file picker will come from the
    // day something has one to ask. `updater` is what lets an application that is a file
    // somebody downloaded replace itself — and it is registered **inert**: nothing in this
    // application asks it anything, and never looking for an update unless a person asked for
    // one is decided at the call that does the asking, not by a builder argument.
    let builder = builder
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build());

    let builder = builder.on_window_event(on_window_event);

    // Where the size a person gave the window is held between the resize that produced it and
    // the exit that writes it down.
    let builder = builder.manage(geometry::Seen::default());

    // Everything this application holds about the agent beside it. Nothing is started here:
    // `setup` below asks only whether this build carries one at all, so that the screen can
    // say which nothing it is showing without fifty megabytes of Python resident for
    // everybody who never opens it.
    let builder = builder.manage(agent::process::Supervisor::new());
    // The node this application runs. Nothing is opened until somebody asks for it.
    let builder = builder.manage(node::Running::default());

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

        // Whether this build carries an agent at all, asked once and nothing started. It is
        // what lets the screen say which nothing it is showing.
        app.state::<agent::process::Supervisor>()
            .look_for_one(app.handle());

        if launch::starts_hidden() {
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
    let builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
        info!("second_launch_folded_into_running_instance");
        window::show_main(app);
    }));

    // Logging comes next so that everything registered below it can write a record.
    // Notifications and the opener sit beside it because order does not matter to either.
    let builder = builder
        .plugin(logging::plugin())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init());

    let builder = assemble_desktop(builder);

    // What the interface may call. The tray is built from this side because its menu is text a
    // person reads, and the catalogs are on the other one.
    builder.invoke_handler(tauri::generate_handler![
        is_development,
        tray::install_tray,
        open_at_login::opens_at_login,
        open_at_login::set_opens_at_login,
        node::node_facts,
        node::open_development_network,
        node::serve_interface,
        node::close_epoch,
        node::join_the_mesh,
        preferences::preferences,
        preferences::set_preferences,
        agent::commands::agent_status,
        agent::commands::agent_ask,
        agent::commands::agent_cancel,
        agent::commands::agent_stop
    ])
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
            info!("application_started");
            app.run(|handle, event| {
                // The size the window ended up at is written down here and nowhere else: it is
                // the one moment at which nothing more can change it.
                //
                // macOS, and only macOS, has a second thing to answer: clicking the Dock icon
                // of a running application with no window on screen is a request for it back.
                // The other two desktops have no Dock and come back through the launcher
                // instead, which the single-instance plugin turns into the very same call.
                match event {
                    tauri::RunEvent::Exit => {
                        // Before the geometry, because ending the agent means closing its
                        // input and waiting a moment for it to go — and a child left running
                        // past this point is one nothing will ever ask to stop again.
                        agent::process::end(handle);
                        geometry::save(handle);
                    }
                    #[cfg(target_os = "macos")]
                    tauri::RunEvent::Reopen { .. } => window::show_main(handle),
                    _ => {}
                }
            });
        }
        Err(error) => panic!("the application could not be built: {error}"),
    }
}
