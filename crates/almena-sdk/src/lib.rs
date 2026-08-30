//! What an issuer and a verifier are built on.
//!
//! # Why this lives in the node's own repository
//!
//! `SPECS.md §13`. The rules an issuer follows when it signs and a verifier follows when it checks
//! are the same rules a node applies when it admits an act, and they are written down once. A
//! library published from somewhere else would agree with the node for about a month, and the day
//! it stopped, the disagreement would be an argument between two projects rather than a failing
//! test in one.
//!
//! # What it does not decide
//!
//! **Whether anybody is to be trusted.** It resolves nothing, fetches nothing and reaches nowhere.
//! Whoever calls it brings the record's answers, and it says what follows from them — because *how
//! many nodes to ask* and *whose seal is worth anything* are the verifier's own policy
//! (`SPECS.md §4.4`, `§7.3`), and a library that decided either would be a library making
//! somebody's trust decisions for them under the name of a default.
//!
//! # And the three answers stay three
//!
//! Everything here that concludes anything concludes it in a vocabulary that keeps *could not be
//! verified* apart from *not valid* (`SPECS.md §17.12`). It is a conformance requirement because
//! conflating them teaches somebody's staff to wave people through when the network fails.

pub mod errand;
pub mod issuer;
pub mod request;
pub mod verifier;
