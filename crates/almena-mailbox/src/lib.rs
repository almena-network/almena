//! What a mediator holds for somebody whose device is off.
//!
//! **The channel everything human travels on** (`SPECS.md §6`): offers, rotations, recovery,
//! the governance of an entity. A mediator is one of the things a node can be switched on to do
//! (`Capability::Mailbox`), and this is the whole of what it decides.
//!
//! - [`quota`] — how much it will hold, and the reserve that makes a flood cost only the flooder.
//! - [`held`] — one message, from the mediator's side of it.
//! - [`account`] — the mailboxes of one person's devices, and their doorbell.
//! - [`mediator`] — every account one node carries post for.
//!
//! # What a mediator knows, and what it does not
//!
//! It knows **which mailboxes belong to one account**, because without that it cannot apply the
//! account's own ceiling — and it reveals nothing new, since how many devices somebody has is
//! already public (`SPECS.md §16-G`). It knows **which relationship a message came from**, because
//! that is what it routes by and what the reserve is counted against; that is metadata inherent to
//! mediated delivery and `SPECS.md §16-S` says so rather than leaving it implicit.
//!
//! It does not know what a message says. Everything here is about **room and time**: how much, how
//! long, and whether the recipient has been by to collect. A mediator that refused on any other
//! ground would be a mediator reading its post.
//!
//! # And a refusal is never silent
//!
//! `SPECS.md §6.2` forbids dropping old messages to make room, because that is how somebody
//! evicts what matters by sending rubbish. The other face of the same attack is that whoever fills
//! the quota makes the legitimate thing bounce — so the sender is told, **and so is the recipient**
//! (`SPECS.md §6.5`): from when it has been full and how many were turned away. Without that a
//! person cannot tell *nobody wrote to me* from *my mailbox is blocked*, which is what makes the
//! attack invisible. It is `SPECS.md §1.2` again: denying service yes, hiding it no.

pub mod account;
pub mod asking;
pub mod held;
pub mod mediator;
pub mod quota;
