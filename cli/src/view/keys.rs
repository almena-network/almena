//! What a key press means, decided apart from anything that draws.
//!
//! This file knows nothing about a terminal, which is what lets every answer below be a test
//! rather than something somebody has to try by hand.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// What the view should do next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Leave, putting the terminal back as it was found.
    Leave,
    /// Close whatever epochs this node owes, without waiting for its own clock to come round.
    CloseEpoch,
    /// Nothing. The key was not one this view answers.
    Stay,
}

/// What `key` asks for.
///
/// `q` leaves, and so do the two things every terminal program is expected to honour —
/// `Ctrl-C` and `Esc`. A person who cannot get out of a full-screen program without reaching
/// for a process list will not run it a second time.
///
/// `e` closes the epoch. It is not `c`: a plain `c` next to `Ctrl-C` is how somebody leaves by
/// accident when their hand was on the modifier a moment ago.
#[must_use]
pub fn action(key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Char('q' | 'Q') | KeyCode::Esc => Action::Leave,
        KeyCode::Char('c' | 'C') if key.modifiers.contains(KeyModifiers::CONTROL) => Action::Leave,
        KeyCode::Char('e' | 'E') => Action::CloseEpoch,
        _ => Action::Stay,
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use super::{Action, action};

    #[test]
    fn q_leaves_in_either_case() {
        for code in [KeyCode::Char('q'), KeyCode::Char('Q')] {
            assert_eq!(action(KeyEvent::from(code)), Action::Leave, "{code:?}");
        }
    }

    #[test]
    fn the_two_conventions_are_honoured() {
        assert_eq!(action(KeyEvent::from(KeyCode::Esc)), Action::Leave);
        assert_eq!(
            action(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            Action::Leave
        );
    }

    #[test]
    fn a_plain_c_is_not_ctrl_c() {
        // The modifier is the whole difference, and reading the code alone is the mistake this
        // catches: somebody typing a `c` must not be thrown out of the view.
        assert_eq!(action(KeyEvent::from(KeyCode::Char('c'))), Action::Stay);
    }

    #[test]
    fn e_closes_the_epoch_in_either_case() {
        for code in [KeyCode::Char('e'), KeyCode::Char('E')] {
            assert_eq!(action(KeyEvent::from(code)), Action::CloseEpoch, "{code:?}");
        }
    }

    #[test]
    fn closing_an_epoch_is_not_next_to_leaving() {
        // `c` sits under the same finger as `Ctrl-C`. Putting an action there would mean somebody
        // whose hand was on the modifier a moment ago leaves without meaning to.
        assert_ne!(
            action(KeyEvent::from(KeyCode::Char('c'))),
            Action::CloseEpoch
        );
    }

    #[test]
    fn everything_else_is_ignored() {
        for code in [KeyCode::Char('x'), KeyCode::Enter, KeyCode::Up] {
            assert_eq!(action(KeyEvent::from(code)), Action::Stay, "{code:?}");
        }
    }
}
