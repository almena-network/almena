//! The epoch: the one clock every node in the network agrees on without coordinating.
//!
//! A node in Santiago and a node in Madrid have to agree on what happened before what, and no
//! amount of goodwill makes two wall clocks agree. Almena settles it in one line — **everything
//! signed, measured or compared is UTC; everything a person reads is in their own zone, and the
//! zone is stated** — and this crate is the first half. Nothing here formats a date for
//! anybody; that belongs to whatever draws a screen.
//!
//! # What an epoch is
//!
//! **The hours elapsed since a fixed genesis instant**, so that two nodes number the same hour
//! the same way with nothing passing between them. The genesis operation fixes that instant
//! for its network, which is why nothing here is a global constant: an epoch number means
//! nothing without the network it belongs to, and [`Clock`] is what carries the pair.
//!
//! # Why every deadline is counted in epochs
//!
//! Because the alternative is calendar arithmetic, and calendar arithmetic is where daylight
//! saving, time zones and months of different lengths get in. Seventy-two hours are seventy-two
//! hours everywhere; *"three days from Tuesday"* is not, and two implementations that disagree
//! about when something expires are two implementations that disagree about whether an
//! operation is valid.
//!
//! So the deadlines of the protocol are [`deadline`] constants here, in epochs, and never
//! durations reconstructed at a call site.
//!
//! # The tolerance, which is not generosity
//!
//! Validation rejects an operation whose declared `emitida` runs ahead of the epoch the node
//! holds as current — but with **one epoch of slack**. Without it a node whose clock is
//! five minutes slow would reject perfectly good operations during those five minutes of every
//! hour, right at the boundary, and two honest nodes would disagree about validity: exactly
//! what the rule exists to prevent. An hour of slack absorbs clock drift and hands nothing
//! useful to anyone declaring a date in the future. See [`Clock::accepts`].

use time::OffsetDateTime;

/// How many epochs make up each deadline the protocol fixes.
///
/// Every one of these is a **versioned protocol parameter** and not a constant buried in code:
/// changing one is adding a rule, never reinterpreting what is already written. They are
/// gathered here so that a change is one edit rather than a search.
pub mod deadline {
    use super::Epochs;

    /// How long an operation signed by the control key alone waits before taking effect, and
    /// during which any live device can cancel it.
    pub const CONTROL_KEY_WAIT: Epochs = Epochs(72);

    /// The shortest expiry a pending signature over a destructive operation may be given, so
    /// that nobody opens one with a thirty-minute window at four in the morning. Three days is
    /// what it takes for an owner who is away to find out.
    pub const DESTRUCTIVE_PENDING_MINIMUM: Epochs = Epochs(72);

    /// How often the seed that places each node in the replication assignment rotates, so that
    /// nobody camps on one object.
    pub const ASSIGNMENT_SEED_ROTATION: Epochs = Epochs(720);

    /// The longest a sender may ask a mediator to hold a message. The sender declares the
    /// expiry and this is the ceiling on it, because a copy waits at every mediator the
    /// recipient declared and the storage is paid for by all of them.
    pub const MESSAGE_MAXIMUM_LIFETIME: Epochs = Epochs(720);

    /// How often a verified domain is checked again. A month bounds how long a domain that has
    /// already changed hands can go on passing for verified, and is frequent enough that a
    /// passing DNS failure reads differently from an abandonment.
    pub const DOMAIN_REVALIDATION: Epochs = Epochs(720);

    /// The notice given before a certification grade is lowered — a month being ample time to
    /// update a node and too little to get used to ignoring the warning.
    pub const GRADE_LOWERING_NOTICE: Epochs = Epochs(720);

    /// The shortest term a public vote may be open for, because the electorate is the whole
    /// network and nobody opens the app daily.
    pub const PUBLIC_VOTE_MINIMUM_TERM: Epochs = Epochs(720);

    /// How long an emergency continuity operation stays open for any surviving owner to veto.
    pub const EMERGENCY_CONTINUITY: Epochs = Epochs(1_440);

    /// How long a mailbox goes uncollected before it is considered inactive and its contents
    /// discarded.
    pub const MAILBOX_INACTIVE: Epochs = Epochs(2_160);

    /// How long an alias is held before it can be taken by anybody else, so that no one
    /// inherits the reputation of whoever left.
    pub const ALIAS_QUARANTINE: Epochs = Epochs(2_160);

    /// How often the opaque wake-up handle a device gives its mediators rotates, which bounds
    /// how long one handle can be used to follow a device about and is how a mediator that
    /// abuses it gets cut off.
    pub const PUSH_HANDLE_ROTATION: Epochs = Epochs(2_160);

    /// How long the key of a relationship lasts before it lapses, which is what makes an
    /// unprocessed rotation heal itself.
    pub const RELATION_KEY_LIFETIME: Epochs = Epochs(8_760);
}

/// A count of epochs — a duration, never a position on the clock.
///
/// Kept apart from [`Epoch`] on purpose: adding two positions is meaningless and subtracting
/// two of them gives a duration, and a type that let both happen would let a deadline be used
/// as a date.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Epochs(pub u64);

impl Epochs {
    /// The count, as a number.
    #[must_use]
    pub const fn count(self) -> u64 {
        self.0
    }
}

/// How many epochs a day is.
///
/// **The day everything is summarised by is a UTC day, and it is this many closed epochs — never
/// the machine's own midnight.** Two observers of one event who each summarised their own midnight
/// would file it in different windows, and comparing summaries is the only thing they exist for.
///
/// It falls out of an epoch being an hour. Nobody chose twenty-four and nobody has to remember it.
pub const EPOCHS_PER_DAY: u64 = 24;

/// A UTC day, counted from the one this network began in.
///
/// Not a date. A date needs a calendar, a calendar needs a wall clock, and nothing here reads one —
/// the instant the network began is the only one anybody wrote down, and everything since is
/// counted from it. Two nodes therefore agree about which day something happened in without
/// agreeing about anything else, including what day it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Day(u64);

impl Day {
    /// The day an epoch falls in.
    #[must_use]
    pub const fn of(epoch: Epoch) -> Self {
        Self(epoch.number() / EPOCHS_PER_DAY)
    }

    /// A day by its number.
    #[must_use]
    pub const fn new(number: u64) -> Self {
        Self(number)
    }

    /// Which day it is, counting from the network's first.
    #[must_use]
    pub const fn number(self) -> u64 {
        self.0
    }

    /// The first epoch of this day.
    #[must_use]
    pub const fn begins(self) -> Epoch {
        Epoch::new(self.0 * EPOCHS_PER_DAY)
    }

    /// Whether every epoch of this day is behind us at `now`.
    ///
    /// **A day is not summarised while it is still happening.** A summary of a day half of which
    /// had not occurred would be comparable with nothing, which is the one thing summaries are for.
    #[must_use]
    pub const fn over(self, now: Epoch) -> bool {
        now.number() >= (self.0 + 1) * EPOCHS_PER_DAY
    }
}

/// A position on the network's clock: the number of whole hours since its genesis instant.
///
/// Two nodes reach the same number for the same moment without exchanging anything, which is
/// the entire point — comparing two roots at *"the same height"* means nothing unless the
/// height is unambiguous, and this is what makes it so.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Epoch(u64);

impl Epoch {
    /// The epoch a genesis instant belongs to, which is zero by definition.
    pub const GENESIS: Self = Self(0);

    /// The epoch a number names.
    ///
    /// Anything may name one: an epoch is a position on a clock everybody computes the same way,
    /// not a thing a node hands out. What a number cannot do is make an epoch real — asking about
    /// one nothing has reached yet is a question with an answer, and the answer is that nothing
    /// was said about it.
    #[must_use]
    pub const fn new(number: u64) -> Self {
        Self(number)
    }

    /// The number of this epoch.
    #[must_use]
    pub const fn number(self) -> u64 {
        self.0
    }

    /// This epoch advanced by a count of epochs, or `None` if the count would overflow.
    ///
    /// Returning an option rather than saturating is deliberate: a deadline that silently
    /// stopped advancing would be a deadline that never falls due, and this crate has no
    /// business deciding that on anybody's behalf.
    #[must_use]
    pub const fn plus(self, epochs: Epochs) -> Option<Self> {
        match self.0.checked_add(epochs.0) {
            Some(sum) => Some(Self(sum)),
            None => None,
        }
    }

    /// How many epochs separate this one from an earlier one, or `None` if the other is later.
    #[must_use]
    pub const fn since(self, earlier: Self) -> Option<Epochs> {
        match self.0.checked_sub(earlier.0) {
            Some(difference) => Some(Epochs(difference)),
            None => None,
        }
    }
}

/// The seconds in an hour, which is the length of an epoch.
const SECONDS_PER_EPOCH: i128 = 3_600;

/// A network's clock: its genesis instant, and the arithmetic that hangs from it.
///
/// Held as a value rather than read from a global because **an epoch number means nothing
/// without its network**. Two networks that shared a numbering would be two networks a node
/// could confuse, and everything about them — down to the name of the protocol two nodes
/// negotiate, which carries the network's identifier inside it — is arranged so they cannot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Clock {
    /// The instant epoch zero begins, fixed by the genesis operation that opened the log.
    genesis: OffsetDateTime,
}

impl Clock {
    /// A clock counting from the instant a genesis operation fixed.
    #[must_use]
    pub const fn from_genesis(genesis: OffsetDateTime) -> Self {
        Self { genesis }
    }

    /// The instant this clock counts from.
    #[must_use]
    pub const fn genesis(&self) -> OffsetDateTime {
        self.genesis
    }

    /// Which epoch an instant falls in, or `None` if it is before genesis.
    ///
    /// Before genesis there is no epoch rather than a negative one: the network did not exist,
    /// and a number that pretended otherwise would be a number somebody could compare.
    #[must_use]
    pub fn epoch_at(&self, instant: OffsetDateTime) -> Option<Epoch> {
        let elapsed =
            (instant.unix_timestamp_nanos() - self.genesis.unix_timestamp_nanos()) / 1_000_000_000;
        if elapsed < 0 {
            return None;
        }
        u64::try_from(elapsed / SECONDS_PER_EPOCH).ok().map(Epoch)
    }

    /// The instant an epoch begins.
    ///
    /// The other direction of [`Self::epoch_at`], and what a node needs to know when the next
    /// root is due — one goes out every epoch whether or not anything happened, because a gap
    /// that means *nothing happened* and a gap that means *I was down* have to be told apart.
    #[must_use]
    pub fn begins(&self, epoch: Epoch) -> Option<OffsetDateTime> {
        let offset = i128::from(epoch.number()).checked_mul(SECONDS_PER_EPOCH)?;
        let nanos = offset.checked_mul(1_000_000_000)?;
        OffsetDateTime::from_unix_timestamp_nanos(self.genesis.unix_timestamp_nanos() + nanos).ok()
    }

    /// Whether an operation declaring `emitida` may be accepted by a node whose current epoch
    /// is `current`.
    ///
    /// The rule every operation is held to: it may not declare a time in the future, **and
    /// one epoch of slack is allowed**. Without the slack a node five minutes slow rejects
    /// good operations for five minutes of every hour, at the boundary — and two honest nodes
    /// disagreeing about validity is the one outcome this design cannot afford. An hour of
    /// slack absorbs drift and returns nothing useful to whoever wanted to post-date something.
    #[must_use]
    pub const fn accepts(declared: Epoch, current: Epoch) -> bool {
        declared.0 <= current.0.saturating_add(1)
    }
}

#[cfg(test)]
mod tests {
    use super::{Clock, Epoch, Epochs, deadline};
    use time::OffsetDateTime;

    /// An arbitrary genesis instant. Nothing depends on which one it is — that is the point of
    /// the clock carrying it.
    fn clock() -> Clock {
        Clock::from_genesis(
            OffsetDateTime::from_unix_timestamp(1_767_225_600).expect("a valid instant"),
        )
    }

    #[test]
    fn a_day_is_twenty_four_closed_epochs_and_not_anybody_s_midnight() {
        // **The reason summaries can be compared at all.** Two observers of one event who each
        // summarised their own midnight would file it in different windows.
        use super::{Day, EPOCHS_PER_DAY};

        assert_eq!(Day::of(Epoch::new(0)).number(), 0);
        assert_eq!(Day::of(Epoch::new(23)).number(), 0, "still the first day");
        assert_eq!(Day::of(Epoch::new(24)).number(), 1);
        assert_eq!(Day::of(Epoch::new(EPOCHS_PER_DAY * 9 + 5)).number(), 9);
    }

    #[test]
    fn a_day_begins_where_it_begins() {
        use super::Day;

        assert_eq!(Day::new(0).begins(), Epoch::new(0));
        assert_eq!(Day::new(3).begins(), Epoch::new(72));
        assert_eq!(Day::of(Epoch::new(77)).begins(), Epoch::new(72));
    }

    #[test]
    fn a_day_is_not_over_while_it_is_still_happening() {
        // A summary of a day half of which had not occurred would be comparable with nothing.
        use super::Day;

        let first = Day::new(0);
        assert!(!first.over(Epoch::new(0)));
        assert!(
            !first.over(Epoch::new(23)),
            "the last hour of it is still it"
        );
        assert!(first.over(Epoch::new(24)));
        assert!(first.over(Epoch::new(9_000)));
    }

    #[test]
    fn genesis_is_epoch_zero() {
        let clock = clock();
        assert_eq!(clock.epoch_at(clock.genesis()), Some(Epoch::GENESIS));
    }

    #[test]
    fn an_epoch_lasts_one_hour() {
        let clock = clock();
        let genesis = clock.genesis();
        let last_second = genesis + time::Duration::seconds(3_599);
        let next = genesis + time::Duration::seconds(3_600);
        assert_eq!(clock.epoch_at(last_second).map(Epoch::number), Some(0));
        assert_eq!(clock.epoch_at(next).map(Epoch::number), Some(1));
    }

    #[test]
    fn before_genesis_there_is_no_epoch() {
        let clock = clock();
        assert_eq!(
            clock.epoch_at(clock.genesis() - time::Duration::seconds(1)),
            None
        );
    }

    #[test]
    fn an_epoch_begins_where_it_is_counted_from() {
        let clock = clock();
        for number in [0_u64, 1, 72, 8_760] {
            let epoch = Epoch::GENESIS.plus(Epochs(number)).expect("no overflow");
            let begins = clock.begins(epoch).expect("a representable instant");
            assert_eq!(clock.epoch_at(begins), Some(epoch));
        }
    }

    #[test]
    fn one_epoch_of_slack_and_not_two() {
        let current = Epoch::GENESIS.plus(Epochs(100)).expect("no overflow");
        let same = current;
        let one_ahead = current.plus(Epochs(1)).expect("no overflow");
        let two_ahead = current.plus(Epochs(2)).expect("no overflow");
        let past = Epoch::GENESIS;

        assert!(
            Clock::accepts(past, current),
            "the past is always acceptable"
        );
        assert!(Clock::accepts(same, current));
        assert!(
            Clock::accepts(one_ahead, current),
            "clock drift is absorbed"
        );
        assert!(!Clock::accepts(two_ahead, current), "the future is not");
    }

    #[test]
    fn a_position_minus_a_position_is_a_duration() {
        let start = Epoch::GENESIS.plus(Epochs(10)).expect("no overflow");
        let later = start.plus(deadline::CONTROL_KEY_WAIT).expect("no overflow");
        assert_eq!(later.since(start), Some(deadline::CONTROL_KEY_WAIT));
        assert_eq!(start.since(later), None, "time does not run backwards");
    }

    #[test]
    fn the_deadlines_are_the_ones_that_were_settled_on() {
        assert_eq!(deadline::CONTROL_KEY_WAIT.count(), 72);
        assert_eq!(deadline::DESTRUCTIVE_PENDING_MINIMUM.count(), 72);
        assert_eq!(deadline::ASSIGNMENT_SEED_ROTATION.count(), 720);
        assert_eq!(deadline::MESSAGE_MAXIMUM_LIFETIME.count(), 720);
        assert_eq!(deadline::DOMAIN_REVALIDATION.count(), 720);
        assert_eq!(deadline::GRADE_LOWERING_NOTICE.count(), 720);
        assert_eq!(deadline::PUBLIC_VOTE_MINIMUM_TERM.count(), 720);
        assert_eq!(deadline::EMERGENCY_CONTINUITY.count(), 1_440);
        assert_eq!(deadline::MAILBOX_INACTIVE.count(), 2_160);
        assert_eq!(deadline::ALIAS_QUARANTINE.count(), 2_160);
        assert_eq!(deadline::PUSH_HANDLE_ROTATION.count(), 2_160);
        assert_eq!(deadline::RELATION_KEY_LIFETIME.count(), 8_760);
    }
}
