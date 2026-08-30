//! Status lists: where a revocation is written down, and when a list is fresh enough to use.
//!
//! # A bit, and only revocation
//!
//! `SPECS.md §10.1`, `§10.2`. The W3C Bitstring Status List, reused rather than invented — the same
//! rule the attribute core follows: **fix the version and copy the definition**. The format admits
//! more states than one; this build writes revocation and leaves the rest as the hole where
//! suspension would go if it were ever needed.
//!
//! # Cohorts by expiry, so a list can be thrown away rather than pruned
//!
//! Every credential expires, so every credential is reissued. Without a rule the index is either
//! recycled — and a new credential inherits the old one's revocation — or never recycled, and the
//! list grows for ever. So **a list covers the credentials that expire inside one window**, and
//! when the window is past the whole list is discarded.
//!
//! It can be discarded **without asking anybody**, and that is the point: expiry is a signed field
//! *inside* the credential and cannot move, while revocation is state the issuer keeps outside it.
//! Everything the list covered is dead, and the credential itself proves it.
//!
//! # And the index is random, because a sequential one is an attribute nobody disclosed
//!
//! A low index says *a customer for a long time*. That travels in every presentation, and the
//! holder never agreed to reveal it. Drawing at random inside a sparse space removes it, satisfies
//! the minimum padding by construction, and costs nothing: a bitstring of almost all zeros
//! compresses away.
//!
//! **What the padding does not do is make the crowd bigger.** Padding bits are not people: an
//! issuer that put forty credentials in a cohort has forty, whether the list is 131 072 bits or two
//! thousand. Calling it an anonymity set would be selling it for more than it is.

pub mod list;
pub mod wanted;
