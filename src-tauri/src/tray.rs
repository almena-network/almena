//! The tray icon, its menu, and the two ways back to a window that is not on screen.
//!
//! This is what makes closing the window something other than quitting. With a tray there the
//! application goes on running with nothing on screen, so this module is also where a person
//! ends it — and [`installed`] is what the window asks before it dares hide, because an
//! application that hides with no tray to come back from is one somebody has lost.
//!
//! # What the menu says, and why in that order
//!
//! With the window put away the tray is the only thing left on screen, and the first question
//! somebody has of it is whether the node is still up. So the menu leads with the state, as an
//! entry that cannot be pressed — it is a reading and not a control — then the way back to the
//! window, and **Quit** last, where a menu's ending entry belongs and where the hand already
//! goes looking for it.
//!
//! **The menu is named by the frontend.** Its entries are text a person reads, and only that
//! side holds the catalogs, so [`install_tray`] is a command rather than a call made at startup:
//! the interface loads, looks its labels up, and hands them here. It is named `install_tray`
//! rather than `install` because a command's name is flat across the whole application: the
//! module in front of it is this side's, not the interface's. The state is a label for the same
//! reason, and it arrives here already read — which network and which of the four the node is —
//! because the four words are a catalogue's and never this side's.

use log::{info, warn};
use tauri::{
    AppHandle, Manager, Wry,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

use crate::window;

/// Identifier of the tray icon, so that it can be asked for rather than remembered.
const TRAY: &str = "main";

/// Identifier of the entry that says what the node is doing.
const STATE: &str = "state";

/// Identifier of the entry that brings the window back.
const SHOW: &str = "show";

/// Identifier of the menu entry that ends the application.
const QUIT: &str = "quit";

/// The entries of the menu, kept so that they can be given their names again.
///
/// The names are text a person reads and therefore come from a catalog, and two things change
/// them while the application runs: the language, and — for the first of them — the node doing
/// something else. [`install_tray`] is called again either way. Holding the entries rather than
/// replacing the menu is what keeps the tray's event wiring untouched: the same items, renamed,
/// and nothing to re-hang the quit behaviour on.
struct Entries {
    /// What the node is doing. Disabled: a reading, not a control.
    state: MenuItem<Wry>,
    /// The way back to the window.
    show: MenuItem<Wry>,
    /// The entry that ends the application.
    quit: MenuItem<Wry>,
}

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

/// Puts the tray icon on the bar, with a menu holding the entries it was handed.
///
/// All three are already translated by the caller — this side has no catalogs to translate them
/// from. `state` is a whole reading rather than a key, for the same reason: which of the four
/// the node is, and which network it is on, are words a catalogue holds.
///
/// **Called again, it renames rather than builds.** Three things call it a second time and all
/// mean the same request: a webview that reloaded, which in development happens on every save;
/// a person changing the language on the Settings screen; and the node doing something else,
/// which is the one that happens while nobody is touching anything. One tray in every case,
/// wearing whatever the interface last called it — a tray left saying *Quit* to somebody
/// reading a Spanish application would be the one piece of English left on screen, and one left
/// saying *Running* over a node that stopped would be worse than saying nothing.
///
/// A failure is written to the log and goes no further. There is nothing the interface could
/// do about it and nothing it could usefully say, and [`installed`] is what keeps the failure
/// from costing anybody their window.
#[tauri::command]
pub fn install_tray(app: AppHandle, state: String, show: String, quit: String) {
    if installed(&app) {
        rename(&app, &state, &show, &quit);
        return;
    }

    if let Err(error) = build(&app, &state, &show, &quit) {
        warn!("tray_not_installed reason={error}");
        return;
    }

    info!("tray_installed");
}

/// Gives the entries their names again: a new language, or a node doing something else.
///
/// A rename that fails leaves an entry saying what it said before, which is a word out of date
/// and not a menu anybody has lost. Each is tried on its own, so one refusing does not cost the
/// other two their new names.
fn rename(app: &AppHandle, state: &str, show: &str, quit: &str) {
    let Some(entries) = app.try_state::<Entries>() else {
        warn!("tray_not_renamed reason=no_entry");
        return;
    };

    for (entry, name) in [
        (&entries.state, state),
        (&entries.show, show),
        (&entries.quit, quit),
    ] {
        if let Err(error) = entry.set_text(name) {
            warn!("tray_not_renamed reason={error}");
        }
    }
}

/// Builds the icon and the menu, and hangs both behaviours off it.
///
/// # Errors
///
/// Returns whatever Tauri raised: the glyph not decoding, the menu not building, or the
/// platform refusing a tray icon — on Linux, most often because nothing on the desktop is
/// serving one.
fn build(app: &AppHandle, state: &str, show: &str, quit: &str) -> tauri::Result<()> {
    // Disabled, and that is what it is for: the first thing anybody wants from a tray is to know
    // the thing is still running, and a reading somebody can press is one they will press.
    let state_item = MenuItem::with_id(app, STATE, state, false, None::<&str>)?;
    let show_item = MenuItem::with_id(app, SHOW, show, true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, QUIT, quit, true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &state_item,
            &PredefinedMenuItem::separator(app)?,
            &show_item,
            &PredefinedMenuItem::separator(app)?,
            &quit_item,
        ],
    )?;

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
            if event.id() == SHOW {
                window::show_main(app);
            } else if event.id() == QUIT {
                info!("quit_from_tray");
                // The node is stopped on the way out, in `RunEvent::Exit`, and not here: there
                // is more than one way to end this application and only one of them is this
                // entry.
                app.exit(0);
            }
        })
        .build(app)?;

    app.manage(Entries {
        state: state_item,
        show: show_item,
        quit: quit_item,
    });

    Ok(())
}
