//! What a node keeps, and what it is allowed to say about it.
//!
//! Three things live here, and the order matters because each rests on the one before:
//!
//! 1. **[`kind`]** — which act an operation is, as the number every log entry carries.
//! 2. **[`chain`]** — the chain each object advances along, and what its history says is true of
//!    it right now. Every rule about who may sign what is here.
//! 3. **[`log`]** — the append-only record of everything this node has accepted, in the order it
//!    accepted it.
//! 4. **[`tree`]** — the Merkle tree over that record. It validates nothing; it gives a position
//!    in time that can be checked.
//! 5. **[`genesis`]** — the one act that opens a network, which a node has to be stopped from
//!    performing when there is somebody to join instead.
//! 6. **[`root`]** — what this node says about its tree at the close of each epoch, signed. Roots
//!    are published and served rather than written to the log: one per node per epoch would be
//!    tens of thousands of entries a day of pure bookkeeping in something nobody ever deletes.
//!
//! # A node stores what it does not understand
//!
//! An operation of a kind this build has never heard of is **stored and passed on**, not refused.
//! Refusing would split the record between versions, and a detector of contradictions cannot tell
//! an out-of-date node from a dishonest one — so it must never be given the chance to confuse
//! them.
//!
//! What such an operation costs is narrow and deliberate: **the object it touches becomes one
//! this node declines to resolve.** Declining service is allowed. Serving the state from before an
//! operation nobody understood, as though it were current, is not — that is a node lying without
//! noticing, which is worse than one that refuses, because nobody is watching it.
//!
//! # The bytes are kept as they arrived
//!
//! Nothing here re-encodes an operation. A signature covers the bytes that were signed, and a node
//! that tidied them before storing would break every signature it touched — including the ones
//! over fields it did not understand well enough to tidy safely.

pub mod announce;
pub mod attribute;
pub mod bind;
pub mod capability;
pub mod certification;
pub mod chain;
pub mod checkpoint;
pub mod contradiction;
pub mod core;
pub mod element;
pub mod entity;
pub mod firm;
pub mod genesis;
pub mod kind;
pub mod log;
pub mod parameter;
pub mod reply;
pub mod resolution;
pub mod root;
pub mod share;
pub mod source;
pub mod summary;
pub mod tag;
pub mod template;
pub mod tree;

/// What an act is about, when that is not its author.
///
/// **One place says this, because two would drift and the drift would be silent.** The log writes
/// an entry with it and anybody checking an inclusion proof rebuilds that entry; the two computing
/// it differently means an honest proof for an honest act is refused, with nothing to look at and
/// nobody at fault.
///
/// [`None`] for everything else, which is most acts: an act about its own author says so by being
/// on that author's chain, and saying it twice would be a hundred bytes in every copy for ever.
#[must_use]
pub fn subject_of(
    operation: &almena_format::operation::Operation,
) -> Option<almena_format::identifier::Did> {
    // A contradiction is about the node that contradicted itself, and a certification is about the
    // party it certifies. Both are found the same way, and it is the way that matters: **by the
    // party affected rather than by whoever bothered to write it down.** An entity that could not
    // ask *what has been said about me* would have to be told by somebody else, which for a seal
    // being withdrawn means finding out because a customer mentions it.
    contradiction::against_whom(operation).or_else(|| certification::about(operation))
}
