//! The main window: bringing it back to the person who asked for it.
//!
//! Desktop only — on a phone the operating system owns the window, and this module is not in
//! the mobile binary.
//!
//! One function today, and it has a module of its own because the reason it exists is not
//! local to any caller: a second launch of the application must not become a second process,
//! so the single-instance plugin turns it into this call instead. The day the window closes to
//! a tray rather than quitting, that is the second caller.

use log::warn;
use tauri::{AppHandle, Manager};

/// Label of the main window, as declared under `app.windows` in `tauri.conf.json`.
const MAIN_WINDOW: &str = "main";

/// Brings the main window back to the user.
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
