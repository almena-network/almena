//! The numbers this record's own rules rest on, and where the type that holds them lives.
//!
//! **A parameter is a history, not a value** — what a number has been, each from the epoch it took
//! effect — and the type is [`almena_time::parameter::Parameter`], beside the clock, because the
//! largest family of these numbers is the deadlines and a deadline is a count of epochs. It is
//! re-exported here so that a module of this crate reaching for one has a single place to reach.
//!
//! What is declared below is what only the record has an opinion about: how much history an object
//! may run up before it owes a summary, and how much the control key may have waiting at once. The
//! deadlines are in [`almena_time::deadline`] and are not repeated here — the same figure written
//! down twice is two figures the day one of them changes.

pub use almena_time::parameter::Parameter;

use almena_time::Epoch;

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
pub const SUMMARISE_EVERY: Parameter = Parameter::from(&[(Epoch::GENESIS, 32)]);

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
pub const CONTROL_PENDING_MOST: Parameter = Parameter::from(&[(Epoch::GENESIS, 64)]);

#[cfg(test)]
mod tests {
    use super::{CONTROL_PENDING_MOST, SUMMARISE_EVERY};
    use almena_time::Epoch;

    #[test]
    fn each_of_them_starts_at_the_genesis() {
        // A parameter with a gap at the beginning would be one with acts nothing could judge.
        for parameter in [SUMMARISE_EVERY, CONTROL_PENDING_MOST] {
            assert_eq!(parameter.settings()[0].0, Epoch::GENESIS, "{parameter:?}");
        }
    }

    #[test]
    fn what_they_are_now_is_what_somebody_about_to_sign_is_held_to() {
        assert_eq!(SUMMARISE_EVERY.now(), 32);
        assert_eq!(CONTROL_PENDING_MOST.now(), 64);
    }
}
