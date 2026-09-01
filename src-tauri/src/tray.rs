//! The tray icon, its menu, and the two ways back to a window that is not on screen.
//!
//! This is what makes closing the window something other than quitting. With a tray there the
//! application goes on running with nothing on screen, so this module is also where a person
//! ends it — and [`installed`] is what the window asks before it dares hide, because an
//! application that hides with no tray to come back from is one somebody has lost.
//!
//! **The menu is named by the frontend.** Its entries are text a person reads, and only that
//! side holds the catalogs, so [`install`] is a command rather than a call made at startup:
//! the interface loads, looks its labels up, and hands them here. It is named `install_tray`
//! rather than `install` because a command's name is flat across the whole application: the
//! module in front of it is this side's, not the interface's.

use log::{info, warn};
use tauri::{
    AppHandle, Manager, Wry,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

use crate::window;

/// Identifier of the tray icon, so that it can be asked for rather than remembered.
const TRAY: &str = "main";

/// Identifier of the menu entry that ends the application.
const QUIT: &str = "quit";

/// The one entry of the menu, kept so that it can be given its name again.
///
/// The name is text a person reads and therefore comes from a catalog, and the language a
/// catalog is read in can change while the application runs — [`install_tray`] is called again
/// when it does. Holding the entry rather than replacing the menu is what keeps the tray's
/// event wiring untouched: the same item, renamed, and nothing to re-hang the quit behaviour on.
struct QuitEntry(MenuItem<Wry>);

/// The glyph a tray draws, and it is not the same file on every platform.
///
/// **The bare mark, which is the same one the holder's client wears.** The two applications are one
/// brand and they show it: the mark is the project's, and neither of them owns a variation on it.
/// There was a version of this that reversed the node's mark inside a dark square so that a machine
/// running both would not put two identical glyphs on one bar — the distinction was real and the
/// cost was not worth it, because a glyph with a ground of its own cannot be a template image, and
/// on macOS that means a tile that ignores the bar it sits in.
///
/// macOS is handed the black one as a template image, which is an alpha mask the system fills to
/// suit its bar, light or dark. The other two do not tint, and a black mark on the dark bar both
/// tend to have is a mark nobody can see — so they are handed the same shape painted in the
/// identity colour, which reads on either.
#[cfg(target_os = "macos")]
const GLYPH: &[u8] = include_bytes!("../icons/tray.png");
#[cfg(all(desktop, not(target_os = "macos")))]
const GLYPH: &[u8] = include_bytes!("../icons/trayColour.png");

/// Whether the tray icon is on the bar.
///
/// The window asks this before hiding instead of closing. If the tray failed to build, hiding
/// would put the application somewhere with no way back on this screen, so the window closes
/// the ordinary way instead and the application ends with it.
pub fn installed(app: &AppHandle) -> bool {
    app.tray_by_id(TRAY).is_some()
}

/// Puts the tray icon on the bar, with a menu holding the entry it was handed.
///
/// `quit` is the name of the one entry, already translated by the caller — this side has no
/// catalogs to translate it from.
///
/// **Called again, it renames rather than builds.** Two things call it a second time and both
/// mean the same request: a webview that reloaded, which in development happens on every save,
/// and a person changing the language on the Settings screen. One tray either way, wearing
/// whatever the interface last called it — a tray left saying *Quit* to somebody reading a
/// Spanish application would be the one piece of English left on screen.
///
/// A failure is written to the log and goes no further. There is nothing the interface could
/// do about it and nothing it could usefully say, and [`installed`] is what keeps the failure
/// from costing anybody their window.
#[tauri::command]
pub fn install_tray(app: AppHandle, quit: String) {
    if installed(&app) {
        rename(&app, &quit);
        return;
    }

    if let Err(error) = build(&app, &quit) {
        warn!("tray_not_installed reason={error}");
        return;
    }

    info!("tray_installed");
}

/// Gives the one entry its name again, in whatever language the interface is now showing.
///
/// A rename that fails leaves the entry saying what it said before, which is a word in the
/// wrong language and not a menu anybody has lost.
fn rename(app: &AppHandle, quit: &str) {
    let Some(entry) = app.try_state::<QuitEntry>() else {
        warn!("tray_not_renamed reason=no_entry");
        return;
    };

    if let Err(error) = entry.0.set_text(quit) {
        warn!("tray_not_renamed reason={error}");
        return;
    }

    info!("tray_renamed");
}

/// Builds the icon and the menu, and hangs both behaviours off it.
///
/// # Errors
///
/// Returns whatever Tauri raised: the glyph not decoding, the menu not building, or the
/// platform refusing a tray icon — on Linux, most often because nothing on the desktop is
/// serving one.
fn build(app: &AppHandle, quit: &str) -> tauri::Result<()> {
    let quit_item = MenuItem::with_id(app, QUIT, quit, true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&quit_item])?;

    TrayIconBuilder::with_id(TRAY)
        .icon(tauri::image::Image::from_bytes(GLYPH)?)
        // A template on macOS, which fills the mask to suit its bar. Elsewhere the glyph is
        // already painted, and saying template would throw the colour away.
        .icon_as_template(cfg!(target_os = "macos"))
        .menu(&menu)
        // The left button is the way back to the window, so it must not be spent opening the
        // menu as well. The menu stays on the right button, where every platform puts it.
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                window::show_main(tray.app_handle());
            }
        })
        .on_menu_event(|app, event| {
            if event.id() == QUIT {
                info!("quit_from_tray");
                app.exit(0);
            }
        })
        .build(app)?;

    app.manage(QuitEntry(quit_item));

    Ok(())
}
