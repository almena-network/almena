//! The main window: bringing it back to the person who asked for it.
//!
//! Desktop only — on a phone the operating system owns the window, and this module is not in
//! the mobile binary.
//!
//! Two functions, and they exist for reasons that are not local to any caller. A second
//! launch of the application must not become a second process, so the single-instance plugin
//! turns it into [`show_main`]; and the close button no longer ends the application, so it
//! turns into [`hide_main`]. Between them they are why the window is somewhere other than on
//! the screen, and why it comes back.

use log::warn;
use tauri::{AppHandle, Manager};

/// Label of the main window, as declared under `app.windows` in `tauri.conf.json`.
const MAIN_WINDOW: &str = "main";

/// Brings the main window back to the user.
///
/// Three callers ask for this and they are the three ways back to a window that is not on
/// screen: launching the application again, which the single-instance plugin folds into this;
/// the tray icon; and, on macOS, the Dock icon of an application whose window is gone.
///
/// Every step is needed and none is worth failing over. The window may be hidden, minimised,
/// or merely behind another application, and each case has its own call; a window that refuses
/// to come forward leaves somebody looking at what they were already looking at, which is not
/// a reason to take the application down.
pub fn show_main(app: &AppHandle) {
    let Some(window) = app.get_webview_window(MAIN_WINDOW) else {
        warn!("window_not_shown reason=no_main_window");
        return;
    };

    for (step, outcome) in [
        ("show", window.show()),
        ("unminimize", window.unminimize()),
        ("focus", window.set_focus()),
    ] {
        if let Err(error) = outcome {
            warn!("window_not_shown step={step} reason={error}");
        }
    }
}

/// Takes the window off the screen without ending the application.
///
/// Two callers: the close button, which no longer means what it means in most applications —
/// the application goes on running and the tray is where it is found — and a launch the system
/// started at login, which puts nothing on the screen at all. `lib.rs` only routes a close here
/// when `tray::installed` says there is a tray to come back from.
///
/// A window that refuses to hide stays on screen. That is a worse-looking application, not a
/// broken one, and taking it down over a refused `hide` would be the actual breakage.
pub fn hide_main(app: &AppHandle) {
    let Some(window) = app.get_webview_window(MAIN_WINDOW) else {
        warn!("window_not_hidden reason=no_main_window");
        return;
    };

    if let Err(error) = window.hide() {
        warn!("window_not_hidden reason={error}");
    }
}
