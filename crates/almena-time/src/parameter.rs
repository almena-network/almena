//! Numbers the protocol can change without reinterpreting anything already written.
//!
//! Some of the figures this design rests on can only be settled by measuring: how many acts an
//! object may write before it owes a summary, how many copies of a thing the network aims to hold,
//! how long something waits. Guessing them once and burying them in the code would mean either
//! living with the guess for ever or breaking the promise that nothing already signed is ever read
//! differently later.
//!
//! # A parameter is a history, not a value
//!
//! So a parameter is not a constant: it is **what it has been, each from the epoch it took
//! effect**. Changing one appends a setting; it never edits one. An act is judged against the
//! value that was in force when its author issued it, so an act that was good when it was written
//! stays good for ever — which is the whole of it, and the reason this exists rather than a `const`.
//!
//! That also makes a change something you can only do **forwards**. A setting that took effect
//! yesterday would decide today what yesterday's acts meant, which is the thing being ruled out.
//!
//! # Why it lives beside the clock
//!
//! Because the deadlines of the protocol are the largest family of these numbers, and a deadline
//! is a count of epochs. Keeping the type anywhere else would mean either the deadlines could not
//! use it — which is how a module comes to say it holds versioned parameters while holding
//! constants — or the crate that owns the clock would have to depend on the crate that owns the
//! record, which is backwards.
//!
//! # What it does not solve
//!
//! Two builds with different histories read the same act differently, and no amount of care here
//! prevents that — it is the ordinary cost of running several versions at once, and what makes it
//! bearable is that a setting is announced far enough ahead that everybody has it before it bites.
//! **A change made without that lead time is a change made wrongly**, and there is nothing in the
//! type that can catch it.

use crate::{Epoch, Epochs};

/// A number that has changed, or may, and what it was each time.
///
/// The settings are earliest first, and the first is at the genesis so that every epoch has an
/// answer. A parameter with a gap at the beginning would be one with acts nothing could judge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Parameter {
    /// What it has been, each from the epoch it took effect.
    settings: &'static [(Epoch, u64)],
}

impl Parameter {
    /// A parameter from what it has been, earliest first.
    ///
    /// The first setting has to be at the genesis, so that every epoch has an answer — one with a
    /// gap at the beginning would be one with acts nothing could judge.
    #[must_use]
    pub const fn from(settings: &'static [(Epoch, u64)]) -> Self {
        Self { settings }
    }

    /// What it was when somebody issued an act at `epoch`.
    ///
    /// The last setting that had taken effect by then — never a later one, which is what makes an
    /// act that was good when written stay good.
    #[must_use]
    pub fn at(self, epoch: Epoch) -> u64 {
        self.settings
            .iter()
            .rev()
            .find(|(from, _)| from.number() <= epoch.number())
            .map_or(self.settings[0].1, |(_, value)| *value)
    }

    /// What it is under the newest setting, for whoever is deciding what to write next.
    ///
    /// **Not for judging anything.** An act is judged against [`Self::at`] its own moment; this is
    /// for the other direction — somebody about to sign, who is subject to the rule as it stands.
    #[must_use]
    pub fn now(self) -> u64 {
        self.settings[self.settings.len() - 1].1
    }

    /// [`Self::at`], as the duration a deadline is.
    ///
    /// A deadline is a count of epochs and never a bare number, and a caller that had to wrap one
    /// itself is a caller that could add it to a position on the clock by mistake.
    #[must_use]
    pub fn epochs(self, epoch: Epoch) -> Epochs {
        Epochs(self.at(epoch))
    }

    /// [`Self::now`], as the duration a deadline is.
    #[must_use]
    pub fn epochs_now(self) -> Epochs {
        Epochs(self.now())
    }

    /// Every setting it has had, earliest first.
    ///
    /// **For checking the shape of one, not for reading its value.** What a parameter was at a
    /// moment is [`Self::at`]; this exists so that the rules a parameter has to keep — starting at
    /// the genesis, running forwards, never twice at one epoch — can be held to by something that
    /// runs rather than by whoever remembers.
    #[must_use]
    pub const fn settings(self) -> &'static [(Epoch, u64)] {
        self.settings
    }
}

#[cfg(test)]
mod tests {
    use super::Parameter;
    use crate::{Epoch, Epochs};

    /// A parameter that changed twice, which is the case worth testing.
    const CHANGED: Parameter = Parameter::from(&[
        (Epoch::GENESIS, 32),
        (Epoch::new(1_000), 16),
        (Epoch::new(2_000), 64),
    ]);

    #[test]
    fn every_epoch_has_an_answer() {
        // A parameter with a gap at the beginning would be one with acts nothing could judge.
        for epoch in [0, 1, 999, 1_000, 1_001, 2_000, u64::MAX] {
            let _ = CHANGED.at(Epoch::new(epoch));
        }
        assert_eq!(CHANGED.at(Epoch::GENESIS), 32);
    }

    #[test]
    fn an_act_is_judged_by_what_was_in_force_when_it_was_issued() {
        // **The whole reason this is not a constant.** An act good when it was written stays good.
        assert_eq!(CHANGED.at(Epoch::new(999)), 32);
        assert_eq!(
            CHANGED.at(Epoch::new(1_000)),
            16,
            "from the epoch it takes effect"
        );
        assert_eq!(CHANGED.at(Epoch::new(1_999)), 16);
        assert_eq!(CHANGED.at(Epoch::new(2_000)), 64);
    }

    #[test]
    fn a_later_setting_never_reaches_back() {
        // Lowering it is the dangerous direction: everything before the change would suddenly owe
        // more than it was asked for. Nothing before the change moves.
        let before: Vec<u64> = (0..1_000).map(|at| CHANGED.at(Epoch::new(at))).collect();
        assert!(
            before.iter().all(|&value| value == 32),
            "every epoch before the change reads what it always read"
        );
    }

    #[test]
    fn what_it_is_now_is_the_newest_setting() {
        // For somebody about to sign, who is subject to the rule as it stands — never for judging.
        assert_eq!(CHANGED.now(), 64);
    }

    #[test]
    fn a_deadline_comes_back_as_a_duration_and_not_as_a_number() {
        // So that nothing can add one to a position on the clock: the two are different types on
        // purpose, and a caller wrapping the number itself is a caller that could get it wrong.
        assert_eq!(CHANGED.epochs(Epoch::new(1_500)), Epochs(16));
        assert_eq!(CHANGED.epochs_now(), Epochs(64));
    }

    #[test]
    fn the_settings_run_forwards_and_never_repeat_an_epoch() {
        // Two settings at one epoch would be two answers to one question, and a setting out of
        // order would be one that decided what earlier acts meant.
        let epochs: Vec<u64> = CHANGED
            .settings()
            .iter()
            .map(|(from, _)| from.number())
            .collect();
        let mut sorted = epochs.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(epochs, sorted);
    }
}
