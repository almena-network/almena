//! How much a mediator will hold, and for whom.
//!
//! **Two rules and not one** (`SPECS.md §6.2`, `§6.5`). There is a ceiling on everything held for
//! one account, summed across its devices' mailboxes; and there is a second one **per
//! relationship**, which is a ceiling from above and a **reserved floor** from below.
//!
//! The floor is the half that is easy to leave out and impossible to do without. With a total cap
//! alone, one counterparty who floods fills the account and every other relationship goes mute
//! while its own channel sits empty — so the way to silence somebody is to write to them, which is
//! the opposite of what a quota is for. `SPECS.md §11.6` had invented an exemption for rotation
//! messages to work around exactly that; the reserve does the same work and asks the mediator only
//! what it already knows, which is **which relationship a message came from**.
//!
//! # Why the reserve is only for small messages
//!
//! A reserve one message can fill is not a reserve. What has to get through when everything else is
//! full is short: a rotation, an acknowledgement, a request to be let back in — so the floor takes
//! small messages and a large one waits for room in the shared part like anything else.
//!
//! # What this deliberately does not decide
//!
//! Whether a message is *allowed* — that is `SPECS.md §6.4`, and it is about who may write to whom.
//! This is only about room. The two are separate because a mediator that refused on grounds of
//! content would be a mediator reading it.

/// The most one message may be.
///
/// **A ceiling on one thing, so the ones below are ceilings on many.** A megabyte is far past any
/// message this protocol carries — an offer, a request, a rotation — and far short of a size that
/// would make the per-relationship ceiling meaningless by filling it in one go.
pub const MESSAGE_MOST: usize = 1024 * 1024;

/// The most one message may be and still be taken out of a relationship's reserved floor.
///
/// **Small, because the floor is for what must get through and not for what is convenient.** A
/// rotation, an acknowledgement, a plea to be let back in: all of them fit in a few kilobytes, and
/// anything larger can wait for room like everything else.
pub const RESERVED_MESSAGE_MOST: usize = 16 * 1024;

/// What one relationship may hold beyond its reserve, in bytes.
pub const RELATION_MOST: usize = 4 * 1024 * 1024;

/// What one relationship holds that nothing else can take.
///
/// Enough for a handful of small messages, which is what the floor is for.
pub const RELATION_RESERVE: usize = 64 * 1024;

/// What one account may hold across every mailbox its devices have, beyond the reserves.
pub const ACCOUNT_MOST: usize = 64 * 1024 * 1024;

/// What the doorbell may hold, for the whole account.
///
/// **Its own channel and its own small quota** (`SPECS.md §6.5`). The root identifier is public and
/// enumerable, so without a separate channel the census would be a list of addressable inboxes and
/// filling it would silence every relationship a person has. Separated, filling it costs them
/// introductions and recovery requests and touches nothing they already have.
///
/// Small because it is for two things — meeting somebody, and asking for help getting back in —
/// and never for anything that belongs to a relationship already established.
pub const DOORBELL_MOST: usize = 256 * 1024;

/// The longest a sender may ask for.
///
/// A copy waits at every mediator the recipient declared, so the storage is paid for by all of
/// them — which is why the sender declares the expiry and does not choose the ceiling.
pub const HELD_AT_MOST: almena_time::parameter::Parameter =
    almena_time::deadline::MESSAGE_MAXIMUM_LIFETIME;

/// How long a mailbox goes uncollected before it is not a mailbox.
pub const UNCOLLECTED_UNTIL_INACTIVE: almena_time::parameter::Parameter =
    almena_time::deadline::MAILBOX_INACTIVE;

/// How much of a message goes where, given what a relationship is already holding.
///
/// **The whole of the reserve rule, in one place.** A message takes what it can out of the
/// relationship's own floor and the rest out of the shared ceiling — so a relationship that has
/// used none of its floor can always take a small message, whatever every other relationship has
/// done to the account's total.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Splits {
    /// What comes out of this relationship's reserved floor.
    pub reserved: usize,
    /// What comes out of the account's shared ceiling.
    pub shared: usize,
}

/// How a message of that size divides, for a relationship already holding `held`.
///
/// Only a small message reaches the floor: a large one takes its whole weight from the shared part,
/// because a floor one message can fill would not be one.
#[must_use]
pub fn splits(size: usize, held: usize) -> Splits {
    let left = RELATION_RESERVE.saturating_sub(held.min(RELATION_RESERVE));
    let reserved = if size <= RESERVED_MESSAGE_MOST {
        size.min(left)
    } else {
        0
    };
    Splits {
        reserved,
        shared: size - reserved,
    }
}

/// What a relationship holding `held` bytes has taken out of the account's shared ceiling.
///
/// Everything past its own floor, and nothing of the floor itself. This is what makes the floor a
/// floor: it is not counted against the total, so it cannot be consumed from elsewhere.
#[must_use]
pub fn beyond_the_reserve(held: usize) -> usize {
    held.saturating_sub(RELATION_RESERVE)
}

#[cfg(test)]
mod tests {
    use super::{RELATION_RESERVE, RESERVED_MESSAGE_MOST, beyond_the_reserve, splits};

    #[test]
    fn a_small_message_comes_out_of_the_floor_while_there_is_floor() {
        // The property the whole rule exists for: a relationship that has used none of its own
        // room can take a small message however full the account is from everywhere else.
        let split = splits(1_000, 0);
        assert_eq!(split.reserved, 1_000);
        assert_eq!(split.shared, 0);
    }

    #[test]
    fn a_large_message_never_touches_the_floor() {
        // A floor one message can fill is not a floor. What has to get through when everything
        // else is full is short, and anything else waits for room like the rest.
        let split = splits(RESERVED_MESSAGE_MOST + 1, 0);
        assert_eq!(split.reserved, 0);
        assert_eq!(split.shared, RESERVED_MESSAGE_MOST + 1);
    }

    #[test]
    fn a_message_that_straddles_the_floor_takes_what_is_left_of_it() {
        let almost = RELATION_RESERVE - 100;
        let split = splits(1_000, almost);
        assert_eq!(split.reserved, 100, "what was left");
        assert_eq!(split.shared, 900, "and the rest from the shared part");
    }

    #[test]
    fn the_floor_is_not_counted_against_the_account() {
        // Which is what makes it a floor rather than a smaller ceiling: what sits inside it cannot
        // be consumed from another relationship, because it was never in the shared total.
        assert_eq!(beyond_the_reserve(0), 0);
        assert_eq!(beyond_the_reserve(RELATION_RESERVE), 0);
        assert_eq!(beyond_the_reserve(RELATION_RESERVE + 7), 7);
    }
}
