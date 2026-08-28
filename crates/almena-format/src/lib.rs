//! The shapes every node and every client agree on: the log entry, the operation, and the name an
//! object gets from its own bytes.
//!
//! The log holds nothing but hashes and the history itself is spread around, so two things have to
//! be kept apart, and this crate keeps them apart too:
//!
//! | | Who holds it | What it is |
//! |---|---|---|
//! | **Operation** | Spread across the network | The whole act, signed |
//! | **Log entry** | **Everyone** | The least that places it in time and says whether it can be interpreted |
//!
//! And one rule gives both their point: **an object is named by the hash of the operation that
//! creates it.** Nobody assigns it, nobody has to be asked for it, and whoever holds the creation
//! recomputes it and checks — without asking any node anything.
//!
//! # Replicated, and held to the same vectors
//!
//! `client` carries its own copy of all of this, because the repositories share no code. What
//! keeps the two from drifting is the golden vectors in `vectors/`, not this file: the same
//! operation written by either program has to come out as the same bytes and the same
//! `did:almena`, or the same words open two different accounts.

pub mod cbor;
pub mod entry;
pub mod field;
pub mod holes;
pub mod identifier;
pub mod operation;
