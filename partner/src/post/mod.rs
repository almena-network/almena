//! What travels between a partner and a holder, written the way the holder's app writes it.
//!
//! A relationship is named by a `did:peer:2` that carries its own keys and where to deliver; a
//! message on it is a DIDComm envelope sealed in the authenticated form, so that opening one
//! proves who sealed it; and the envelope is handed to a mediator that holds bytes and reads
//! nothing. None of it is this project's invention, and every piece is held to the vector the
//! standard that defines it publishes — because the other end of this wire is a different program
//! and the two agree by the numbers, not by sharing code.

pub mod envelope;
pub mod keywrap;
pub mod mediator;
pub mod message;
pub mod peer;
pub mod sealing;
