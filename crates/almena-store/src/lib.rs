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
pub mod chain;
pub mod checkpoint;
pub mod contradiction;
pub mod firm;
pub mod genesis;
pub mod kind;
pub mod log;
pub mod root;
pub mod summary;
pub mod tree;
