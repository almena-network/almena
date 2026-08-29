//! Numbers the protocol can change without reinterpreting anything already written.
//!
//! Some of the figures this design rests on can only be settled by measuring: how many acts an
//! object may write before it owes a summary, how many copies of a thing the network aims to hold.
//! Guessing them once and burying them in the code would mean either living with the guess for
//! ever or breaking the promise that nothing already signed is ever read differently later.
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
//! # What it does not solve
//!
//! Two builds with different histories read the same act differently, and no amount of care here
//! prevents that — it is the ordinary cost of running several versions at once, and what makes it
//! bearable is that a setting is announced far enough ahead that everybody has it before it bites.
//! **A change made without that lead time is a change made wrongly**, and there is nothing in the
//! type that can catch it.

use almena_time::Epoch;

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
}

/// How many acts an object may add without summarising before the next one has to carry the
/// summary.
///
/// **Thirty-two, and it is a starting value.** What a summary saves is acts to replay, so the
/// figure is not a balance between two harms: a summary is *added* to a chain and never replaces
/// anything, so it makes the history bigger and the reading shorter. Thirty-two puts the overhead
/// of a chain at roughly a tenth and leaves whoever arrives a handful of acts per object.
///
/// It is the kind of number that can only be settled with the network running — if starting a node
/// turns out to be heavy it comes down, and if what hurts is storage it goes up.
pub const SUMMARISE_EVERY: Parameter = Parameter {
    settings: &[(Epoch::GENESIS, 32)],
};

/// How long an act signed by the control key alone waits before it takes effect.
///
/// **Seventy-two epochs, and it is the counterweight the devices hold.** The control key comes
/// from words, and words can be read over a shoulder, photographed, or coerced — so what that key
/// signs alone does not land at once. It enters the record immediately, where every current
/// device can see it, and any of them can cancel it before the wait runs out. Whoever holds only
/// the words and none of the devices pays the wait; whoever stole the words finds the owner's
/// devices still standing between them and the account.
///
/// It costs nothing to somebody with a live device, because a device signs immediately — the wait
/// only ever binds the words signing alone.
pub const CONTROL_WAITS: Parameter = Parameter {
    settings: &[(Epoch::GENESIS, 72)],
};

/// How many acts the control key may have waiting on one account at once.
///
/// **The words queue a handful of things and no more.** A person recovering with their words adds
/// a device or two, removes one they lost, perhaps rotates — each landing after its wait or struck
/// out by a device. Nothing honest reaches into the dozens. The cap is here for the other case: an
/// asking the words sign alone enters the record and waits, and without a ceiling somebody holding
/// only the words could sign thousands, each one an entry every reader clones and walks on every
/// act after it — a chain that costs the square of its length to take in. Refused past the cap, the
/// asking is not stored, so every node refuses it alike and none diverges.
///
/// Sixty-four is far above any honest use and far below where the cost bites. It is a starting
/// value, versioned like the rest: if it ever pinches it goes up.
pub const CONTROL_PENDING_MOST: Parameter = Parameter {
    settings: &[(Epoch::GENESIS, 64)],
};

#[cfg(test)]
mod tests {
    use super::{Parameter, SUMMARISE_EVERY};
    use almena_time::Epoch;

    /// A parameter that changed twice, which is the case worth testing.
    const CHANGED: Parameter = Parameter {
        settings: &[
            (Epoch::GENESIS, 32),
            (Epoch::new(1_000), 16),
            (Epoch::new(2_000), 64),
        ],
    };

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
        // more summaries than it wrote. Nothing before the change moves.
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
        assert_eq!(SUMMARISE_EVERY.now(), 32);
    }

    #[test]
    fn the_settings_run_forwards_and_never_repeat_an_epoch() {
        // Two settings at one epoch would be two answers to one question, and a setting out of
        // order would be one that decided what earlier acts meant.
        for parameter in [SUMMARISE_EVERY, CHANGED] {
            let epochs: Vec<u64> = parameter
                .settings
                .iter()
                .map(|(from, _)| from.number())
                .collect();
            let mut sorted = epochs.clone();
            sorted.sort_unstable();
            sorted.dedup();
            assert_eq!(epochs, sorted, "{parameter:?}");
        }
    }

    #[test]
    fn it_starts_at_the_genesis() {
        for parameter in [SUMMARISE_EVERY, CHANGED] {
            assert_eq!(parameter.settings[0].0, Epoch::GENESIS, "{parameter:?}");
        }
    }
}
