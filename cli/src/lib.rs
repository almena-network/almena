//! Almena, the node for a computer that has no graphical system to run one from.
//!
//! The windowed application is a node on a computer with a screen. This is the same thing for
//! a machine in a rack: it brings a node up and keeps it up, and it draws what that node is
//! for whoever is watching. It is **not** the windowed application without a window — it has
//! no settings, no appearance, no notifications, no tray and no login entry, because every one
//! of those is something a person operates and the answer for a server is its own operating
//! system.
//!
//! Everything but the launch lives here rather than in `main.rs`, so that a test can link
//! against it — the same arrangement `almena-app` uses and for the same reason.
//!
//! See spec `0001` for what this is and what it deliberately is not.

pub mod arguments;
pub mod catalog;
pub mod language;
pub mod node;
pub mod records;
pub mod view;

use clap::Parser as _;
use log::{error, info};

use crate::arguments::Arguments;
use crate::catalog::Catalog;
use crate::language::Language;
use crate::node::Node;

/// What this program calls itself, and therefore the name of every directory it keeps.
///
/// **Not the windowed application's.** That one is `network.almena.desktop`, declared in
/// `tauri.conf.json`, and the two are deliberately different: they are two programs, so they
/// keep separate directories, so they hold separate keys, so a machine running both is two
/// nodes. Spec `0001` records that as a decision rather than leaving it to be discovered.
pub const IDENTIFIER: &str = "network.almena.cli";

/// The name this program's log files carry — `.agents/rules/logging.md`.
///
/// The binary a person types, and not the package, which is `almena-cli`. Two programs never
/// share a file, and `almena-app` is the other one's.
pub const PROGRAM: &str = "almena";

/// Brings a node up, draws it or writes about it, and takes it down again.
///
/// Returns the code the process should exit with: `0` unless the terminal itself failed.
#[must_use]
pub fn run() -> u8 {
    let arguments = Arguments::parse();
    let writes_records = arguments.writes_records();

    let directories = almena_paths::Paths::for_application(IDENTIFIER);
    let logs = directories.logs().ok();
    let destination = records::install(PROGRAM, logs.as_deref(), writes_records);

    let node = Node::start(destination);
    let mut code = 0;

    if writes_records {
        // Nothing to draw and nobody to draw it for: the node is up, and it stays up until the
        // operating system says otherwise.
        info!("waiting_for_stop");
        wait_for_stop();
    } else {
        let catalog = Catalog::of(Language::from_environment());
        if let Err(failure) = view::run(&node, &catalog) {
            error!("view_failed reason={failure}");
            code = 1;
        }
    }

    node.stop();
    code
}

/// Blocks until the operating system asks this process to stop.
///
/// `Ctrl-C` at a terminal and the `SIGTERM` a service manager sends both arrive here, so a
/// node stopped either way says so in its records instead of vanishing mid-line.
fn wait_for_stop() {
    let (tell, wait) = std::sync::mpsc::channel();

    if ctrlc::set_handler(move || {
        // A failed send means the receiver is gone, which means the program is already on its
        // way out. There is nothing to report and nobody to report it to.
        let _ = tell.send(());
    })
    .is_err()
    {
        error!("stop_signal_not_installed");
        return;
    }

    // A failed receive means every sender was dropped, which cannot happen while the handler
    // holds one — but it is still a reason to stop waiting rather than to wait for ever.
    let _ = wait.recv();
    info!("stop_requested");
}
