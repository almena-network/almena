//! The size a person gave the window, remembered in **points** rather than in pixels.
//!
//! The window-state plugin already remembers a geometry between runs, and it remembers it in
//! pixels — what it writes down is `inner_size()`, which is physical. On a computer with one
//! display that is the same number as the size a person chose and there is nothing here to say.
//! On a computer with two displays of different scale it is not: a window somebody sized to 900
//! points on a display that draws two pixels to the point is written down as 1800, and given
//! back on a display that draws one pixel to the point it comes back 1800 points wide — twice
//! the window they left.
//!
//! So the size is also kept here, in the unit the person actually chose it in, and the window
//! is corrected once at startup and **only when the two disagree**. That last part is why the
//! plugin keeps its `SIZE` flag rather than losing it: on the ordinary launch, the window coming
//! back to the display it was on, this module compares two numbers and resizes nothing, where a
//! module that owned the size outright would resize every window on every launch and be seen
//! doing it.
//!
//! **Dragging a window between two displays needs nothing from this module**, and that is worth
//! writing down because it is the first thing anybody looks for here. The window keeps its size
//! in points across a change of scale already — `tao` recomputes it on all three desktops
//! rather than accepting whatever the platform suggested — so the window a person drags to a
//! second display is the same window. What changes is how large a point is on that display, and
//! that is the operating system's to decide and not an application's to undo.

use std::{fs, path::PathBuf, sync::Mutex};

use log::{info, warn};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, LogicalSize, Manager, PhysicalSize, Window};

/// The file the size is kept in.
///
/// Beside the log rather than beside the configuration, because a window's geometry is state:
/// worth keeping between runs, not worth backing up, and never required to start. Deleting it
/// costs the remembered size and nothing else, and the window then opens at the size
/// `tauri.conf.json` declares.
const FILE: &str = "window.json";

/// How far two sizes may differ, in points, before the difference is worth acting on.
///
/// A point, because a rounding of physical pixels back into points can land just off the number
/// that was written down, and resizing a window to correct a third of a point would be a window
/// that twitches for nothing.
const TOLERANCE: f64 = 1.0;

/// An inner size, in logical points.
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
struct Size {
    /// Width, in points.
    width: f64,
    /// Height, in points.
    height: f64,
}

/// The size the window was last seen at, until [`save`] writes it out.
///
/// Held rather than written on every event for the reason the plugin holds its own: a drag of a
/// window edge is hundreds of resizes, and a file written on each of them is a file written
/// hundreds of times to record one decision.
#[derive(Default)]
pub struct Seen(Mutex<Option<Size>>);

/// Where the file is, or nothing when the platform will not say.
fn file(app: &AppHandle) -> Option<PathBuf> {
    match app.path().app_log_dir() {
        Ok(directory) => Some(directory.join(FILE)),
        Err(error) => {
            warn!("window_size_not_located reason={error}");
            None
        }
    }
}

/// Whether the window is in a state whose size is not the one to remember.
///
/// Maximised, full screen and minimised are all sizes the platform chose, and giving one of them
/// back as the windowed size is how a window that was maximised once is never a window again.
fn platform_owns_size(window: &Window) -> bool {
    window.is_minimized().unwrap_or(false)
        || window.is_maximized().unwrap_or(false)
        || window.is_fullscreen().unwrap_or(false)
}

/// Records the size the window now has, in points.
///
/// Called on every resize, which includes the resize a change of scale produces: the physical
/// size changes there and the logical one does not, so the number recorded is the same one and
/// the record stays true across a drag between displays.
///
/// `size` is the inner size the event carried, in physical pixels.
pub fn note(window: &Window, size: PhysicalSize<u32>) {
    if size.width == 0 || size.height == 0 || platform_owns_size(window) {
        return;
    }

    let Ok(scale) = window.scale_factor() else {
        return;
    };

    if scale <= 0.0 {
        return;
    }

    let Some(seen) = window.try_state::<Seen>() else {
        return;
    };

    let logical = size.to_logical::<f64>(scale);

    if let Ok(mut held) = seen.0.lock() {
        *held = Some(Size {
            width: logical.width,
            height: logical.height,
        });
    }
}

/// Writes the size out, so that the next run can give it back.
///
/// Nothing seen means nothing written, which is what keeps a run in which nobody touched the
/// window from replacing a size somebody chose in the run before it.
pub fn save(app: &AppHandle) {
    let Some(seen) = app.try_state::<Seen>() else {
        return;
    };

    let Ok(held) = seen.0.lock() else {
        return;
    };

    let Some(size) = *held else {
        return;
    };

    if let Err(error) = write(app, size) {
        warn!("window_size_not_stored reason={error}");
    }
}

/// Puts a size on disk.
///
/// # Errors
///
/// Returns whatever the filesystem raised: the directory not being creatable, or the file not
/// being writable.
fn write(app: &AppHandle, size: Size) -> std::io::Result<()> {
    let Some(path) = file(app) else {
        return Ok(());
    };

    if let Some(directory) = path.parent() {
        fs::create_dir_all(directory)?;
    }

    let text =
        serde_json::to_string(&size).map_err(|error| std::io::Error::other(error.to_string()))?;

    fs::write(path, text)
}

/// The size that was written down, or nothing when none was.
fn read(app: &AppHandle) -> Option<Size> {
    let path = file(app)?;
    let text = fs::read_to_string(path).ok()?;

    match serde_json::from_str::<Size>(&text) {
        Ok(size) if size.width > 0.0 && size.height > 0.0 => Some(size),
        Ok(_) => None,
        Err(error) => {
            warn!("window_size_not_understood reason={error}");
            None
        }
    }
}

/// Gives the window back the size it was given, when what it has is not that.
///
/// Run once at startup, after the plugin has put the window where it was. By then the window is
/// on the display it will open on, so its scale factor is that display's and the comparison
/// below is between two sizes in the same unit on the same screen.
pub fn restore(window: &Window) {
    let Some(wanted) = read(window.app_handle()) else {
        return;
    };

    if platform_owns_size(window) {
        return;
    }

    let (Ok(scale), Ok(size)) = (window.scale_factor(), window.inner_size()) else {
        return;
    };

    let now = size.to_logical::<f64>(scale);

    if (now.width - wanted.width).abs() < TOLERANCE
        && (now.height - wanted.height).abs() < TOLERANCE
    {
        return;
    }

    if let Err(error) = window.set_size(LogicalSize::new(wanted.width, wanted.height)) {
        warn!("window_size_not_restored reason={error}");
        return;
    }

    info!(
        "window_size_restored width={} height={} scale={scale}",
        wanted.width, wanted.height
    );
}
