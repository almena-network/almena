//! Whether the operating system opens this application when somebody logs in.
//!
//! Desktop only, and served by two different mechanisms, because macOS is the one platform that
//! keeps two registers and means genuinely different things by them:
//!
//! - **Open at Login** is the application itself, opened for the person who has just logged in,
//!   listed under its own name and its own icon. That is what this setting is.
//! - **Allow in the Background** is a helper the system keeps running on its own account. A
//!   `LaunchAgent` lands there — and a `LaunchAgent` is what a general-purpose autostart library
//!   writes on macOS, which is why writing one is the wrong answer here even though it does
//!   start the application.
//!
//! Windows and Linux keep one register each and draw no such distinction:
//! `HKCU\…\CurrentVersion\Run` and `~/.config/autostart`, both written by
//! `tauri-plugin-autostart`. macOS goes through `SMAppService`, which is the only API that
//! registers the first without asking a person for permission to drive System Events.
//!
//! **Running in the tray is not this setting and never was.** That is what the application does
//! once it is running, whoever started it — see `tray.rs`. This decides only who starts it.

use log::info;
use tauri::AppHandle;

/// Whether the system is set to open this application when somebody logs in.
#[tauri::command]
pub fn opens_at_login(app: AppHandle) -> bool {
    platform::enabled(&app)
}

/// Turns opening at login on or off, and reports what the system says **afterwards**.
///
/// The answer is read back rather than echoing what was asked, and that is the whole point of
/// the return value: setting it can succeed and change nothing — a policy that forbids login
/// items, a macOS registration a person has since switched off in System Settings — and a
/// caller that trusted the request would draw a switch that had moved with nothing behind it.
/// Comparing this against what was asked is how the interface knows.
#[tauri::command]
pub fn set_opens_at_login(app: AppHandle, wanted: bool) -> bool {
    platform::set(&app, wanted);

    let now = platform::enabled(&app);
    info!("open_at_login wanted={wanted} now={now}");
    now
}

/// Windows and Linux, through the register each of them keeps.
#[cfg(not(target_os = "macos"))]
mod platform {
    use log::warn;
    use tauri::AppHandle;
    use tauri_plugin_autostart::ManagerExt;

    /// Whether the register holds an entry for this application.
    pub fn enabled(app: &AppHandle) -> bool {
        app.autolaunch().is_enabled().unwrap_or_else(|error| {
            warn!("open_at_login_unreadable reason={error}");
            false
        })
    }

    /// Writes the entry, or takes it out.
    ///
    /// Says nothing about whether it worked. The caller reads the register back, which is a
    /// better answer than this one could give: on both platforms the write can be accepted and
    /// then undone by something else.
    pub fn set(app: &AppHandle, wanted: bool) {
        let outcome = if wanted {
            app.autolaunch().enable()
        } else {
            app.autolaunch().disable()
        };

        if let Err(error) = outcome {
            warn!("open_at_login_not_set reason={error}");
        }
    }

    #[cfg(test)]
    mod tests {
        use super::{SMAppService, SMAppServiceStatus};

        /// Asking is safe from a binary that is not an application bundle.
        ///
        /// This matters because it is the situation `task dev` puts the code in, and because
        /// the binding returns a non-optional object: were the framework to hand back nothing
        /// there, `objc2` would trap rather than return, and a switch on a settings screen
        /// would take the application down instead of drawing itself as off. A test binary is
        /// not a bundle either, so running this at all is the check.
        #[test]
        fn status_is_readable_outside_a_bundle() {
            // SAFETY: as in `enabled` above — a class method with no arguments, then an
            // instance method on the object it returned, which is alive across the call.
            let status = unsafe { SMAppService::mainAppService().status() };

            assert!(
                [
                    SMAppServiceStatus::NotRegistered,
                    SMAppServiceStatus::Enabled,
                    SMAppServiceStatus::RequiresApproval,
                    SMAppServiceStatus::NotFound,
                ]
                .contains(&status),
                "unknown SMAppService status: {status:?}"
            );
        }
    }
}

/// macOS, through `SMAppService`.
///
/// # Unsafe
///
/// `unsafe_code` is denied across this workspace and lifted for this module alone, which is
/// what the rule allows and how it is meant to be done. Every call below crosses into
/// Objective-C, where the Rust compiler can check nothing at all, so each carries the reason it
/// is sound. There are four of them and there will not be more: the whole of what this
/// application asks macOS is *is it registered*, *register it*, *unregister it*.
///
/// **It only works from a bundle.** `SMAppService` registers the application enclosing the
/// running executable, so a binary run straight out of `target/` has no bundle to register and
/// every call here fails. That is `task dev` on macOS, and it is expected rather than broken.
#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
mod platform {
    use log::warn;
    use objc2_service_management::{SMAppService, SMAppServiceStatus};
    use tauri::AppHandle;

    /// Whether macOS lists this application under *Open at Login*.
    ///
    /// Only `Enabled` counts. `RequiresApproval` means macOS has the registration and the person
    /// switched it off in System Settings, which is a decision of theirs to respect and report
    /// as off — it is logged, because it is the one case where a switch that will not move has
    /// an explanation somewhere else on the screen.
    pub fn enabled(_app: &AppHandle) -> bool {
        // SAFETY: `mainAppService` is a class method taking no arguments. It reads the bundle
        // enclosing the running executable and returns a retained object or nothing we could
        // misuse; there is no pointer of ours involved.
        let service = unsafe { SMAppService::mainAppService() };

        // SAFETY: an instance method on the object just returned, taking no arguments and
        // returning a plain integer. `service` is alive for the whole call.
        let status = unsafe { service.status() };

        if status == SMAppServiceStatus::RequiresApproval {
            warn!("open_at_login_needs_approval");
        }

        status == SMAppServiceStatus::Enabled
    }

    /// Registers this application as a login item, or takes the registration away.
    pub fn set(_app: &AppHandle, wanted: bool) {
        // SAFETY: as above — a class method with no arguments and nothing of ours passed in.
        let service = unsafe { SMAppService::mainAppService() };

        // SAFETY: both are instance methods on a live object, taking no arguments and returning
        // either success or a retained `NSError`. The binding turns that pair into a `Result`,
        // so there is no out-pointer for this code to get wrong.
        let outcome = if wanted {
            unsafe { service.registerAndReturnError() }
        } else {
            unsafe { service.unregisterAndReturnError() }
        };

        if let Err(error) = outcome {
            warn!("open_at_login_not_set reason={error}");
        }
    }

    #[cfg(test)]
    mod tests {
        use super::{SMAppService, SMAppServiceStatus};

        /// Asking is safe from a binary that is not an application bundle.
        ///
        /// This matters because it is the situation `task dev` puts the code in, and because
        /// the binding returns a non-optional object: were the framework to hand back nothing
        /// there, `objc2` would trap rather than return, and a switch on a settings screen
        /// would take the application down instead of drawing itself as off. A test binary is
        /// not a bundle either, so running this at all is the check.
        #[test]
        fn status_is_readable_outside_a_bundle() {
            // SAFETY: as in `enabled` above — a class method with no arguments, then an
            // instance method on the object it returned, which is alive across the call.
            let status = unsafe { SMAppService::mainAppService().status() };

            assert!(
                [
                    SMAppServiceStatus::NotRegistered,
                    SMAppServiceStatus::Enabled,
                    SMAppServiceStatus::RequiresApproval,
                    SMAppServiceStatus::NotFound,
                ]
                .contains(&status),
                "unknown SMAppService status: {status:?}"
            );
        }
    }
}
