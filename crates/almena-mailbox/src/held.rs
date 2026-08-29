//! One message, as the mediator holds it.
//!
//! **Bytes, a name, a relationship and a deadline.** What is inside is between the two ends
//! (`SPECS.md §6.1`), and nothing here opens it: a mediator that read its post would be a mediator
//! whose promise is worth what its operator's word is worth.
//!
//! The name matters more than it looks. A sender delivers to **every mediator the recipient
//! declared** (`SPECS.md §6.2`), so the same message arrives more than once and the recipient's
//! side has to tell one message arriving twice from two messages. It tells them by the name — and
//! the name is the hash of the sealed bytes, so it is the same at every mediator without anybody
//! having to agree on it, and it is nobody's to choose.
//!
//! **Nobody's to choose is the part that matters.** A name a sender picked would be a name a sender
//! could pick to collide with somebody else's message, and a collision is a message that vanishes
//! into the recipient's deduplication without either end ever knowing. Hashing the bytes takes that
//! lever away: two messages have one name exactly when they are the same message.

use almena_format::identifier::Name;
use almena_time::{Epoch, Epochs};

/// One message waiting to be collected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Held {
    /// What it is called: the hash of the sealed bytes.
    ///
    /// **The recipient's means of telling one message from two.** The same message is delivered to
    /// every mediator on the recipient's list, so a client collecting from three of them sees it
    /// three times. Being the hash, it is the same at all three without anybody agreeing on it, a
    /// mediator that altered the bytes cannot keep it, and no sender can aim it at somebody else's
    /// message.
    pub called: Name,
    /// Which relationship it came from, which is what the reserve is counted against.
    ///
    /// A peer identifier and never a root one: the root is public and enumerable, and what arrives
    /// addressed to it goes to the doorbell instead.
    pub relation: String,
    /// The message itself, sealed between its two ends.
    pub sealed: Vec<u8>,
    /// The first epoch at which it is no longer held.
    pub until: Epoch,
}

impl Held {
    /// One message, with the expiry the sender asked for held to the ceiling.
    ///
    /// **The sender declares and the ceiling is not theirs to choose** (`SPECS.md §6.2`): a copy
    /// waits at every mediator the recipient declared, so a month asked for once is a month paid
    /// for several times over. Asking for longer is not refused — it is shortened, because the
    /// message is still worth delivering and the sender is not the one being protected against.
    #[must_use]
    pub fn new(relation: String, sealed: Vec<u8>, asked: Epochs, at: Epoch) -> Self {
        let asked = Epochs(asked.count().min(crate::quota::HELD_AT_MOST.count()));
        Self {
            called: Name::of(&sealed),
            relation,
            sealed,
            // A deadline past the end of the clock is one nothing can reach, which is the same
            // answer as the longest this will hold — and never a panic.
            until: at.plus(asked).unwrap_or(Epoch::new(u64::MAX)),
        }
    }

    /// What it costs the mailbox that holds it.
    #[must_use]
    pub fn weighs(&self) -> usize {
        self.sealed.len()
    }

    /// Whether it is past the moment it was to be held until.
    #[must_use]
    pub fn expired(&self, at: Epoch) -> bool {
        at.number() >= self.until.number()
    }
}
