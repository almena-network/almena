//! Putting a notification on the device's screen from the Rust side.
//!
//! The frontend reaches the same plugin its own way and does not come through here. It holds
//! the translation catalogs, so it is the side that can say what it is announcing; this module
//! is for the code that runs with no webview in front of it — before a window exists, after one
//! has gone, or on a launch that never opens one.
//!
//! **Nothing in this repository calls it yet.** There is no network, so there is nothing to
//! announce, and a caller invented to exercise it would be announcing something untrue. It is
//! here so that the code that does have something to say says it through one function instead
//! of reaching into a plugin.

use log::{info, warn};
use tauri::AppHandle;
use tauri_plugin_notification::NotificationExt;

/// Shows a notification, and says whether the platform accepted it.
///
/// `title` is the first line and `body` the rest. Both are read by a person, so neither is
/// ever a literal in the source ([`user-facing-text`]): they arrive already translated, from a
/// caller that took them out of the catalogs. That is why this takes text and not a key —
/// only the frontend holds the catalogs, and this function is for the side that does not. The
/// operating system draws the application's own name beside the title, so repeating it there
/// spends the line on nothing.
///
/// Returns `false` when the platform refused: permission not granted, or nothing running to
/// draw it. A caller has nothing to retry and nowhere to report it — the one way to tell a
/// person something with no window in front of them is exactly what just failed — so the
/// reason goes to the log and the caller carries on.
///
/// # Examples
///
/// ```no_run
/// # use tauri::AppHandle;
/// # fn announce(app: &AppHandle, title: &str, body: &str) {
/// almena_app_lib::notification::show(app, title, body);
/// # }
/// ```
///
/// [`user-facing-text`]: https://github.com/almena-network/almena-network/blob/main/.agents/rules/user-facing-text.md
pub fn show(app: &AppHandle, title: &str, body: &str) -> bool {
    match app.notification().builder().title(title).body(body).show() {
        Ok(()) => {
            info!("notification_shown");
            true
        }
        Err(error) => {
            warn!("notification_refused reason={error}");
            false
        }
    }
}
