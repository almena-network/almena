//! The live view: the terminal, the loop, and putting the terminal back as it was found.
//!
//! Split three ways on purpose. [`draw`] decides what is on the screen and touches no
//! terminal; [`keys`] decides what a key press means and touches no terminal; this file is
//! the only part that does, and it is therefore the only part that cannot be a test.

pub mod draw;
pub mod keys;

use std::time::Duration;

use crossterm::event::{self, Event};
use log::info;

use crate::catalog::Catalog;
use crate::node::Node;

/// How long to wait for a key before drawing again.
///
/// Nothing this view shows changes on its own yet, so this is only how quickly it answers a
/// key. It is short enough not to feel stuck and long enough to leave a sleeping process
/// asleep, which matters on a machine that is meant to be a node and not a space heater.
const TICK: Duration = Duration::from_millis(250);

/// Draws the node until somebody leaves, then restores the terminal.
///
/// # Errors
///
/// Returns whatever the terminal did wrong — entering the alternate screen, drawing, or
/// reading an event. **The terminal is restored either way**: a program that leaves somebody's
/// shell without an echo because it failed is worse than one that failed.
pub fn run(node: &Node, catalog: &Catalog) -> std::io::Result<()> {
    let mut terminal = ratatui::init();
    info!("view_opened");

    let outcome = loop {
        if let Err(error) = terminal.draw(|frame| draw::draw(frame, node, catalog)) {
            break Err(error);
        }

        match event::poll(TICK) {
            Ok(false) => continue,
            Err(error) => break Err(error),
            Ok(true) => {}
        }

        match event::read() {
            Err(error) => break Err(error),
            // A key event arrives twice on Windows — once pressed, once released — and
            // answering both would leave on the press and then act on a terminal that is
            // already restored.
            Ok(Event::Key(key)) if key.is_press() => match keys::action(key) {
                keys::Action::Leave => break Ok(()),
                // Nothing is drawn about it beyond the count going up on the next frame: what a
                // node owes and what it has closed are facts it reports, and a face that answered
                // this one itself would be a face computing a fact.
                keys::Action::CloseEpoch => {
                    let _ = node.close_epoch();
                }
                keys::Action::Stay => {}
            },
            Ok(_) => {}
        }
    };

    ratatui::restore();
    info!("view_closed");
    outcome
}
