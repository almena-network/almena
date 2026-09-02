//! The reference issuer and verifier, as a program.
//!
//! # What it is for
//!
//! The library one crate over says what an issuer signs and what a verifier concludes, and a test
//! there walks a credential from issuance to refusal inside one process. This is that test with a
//! wire in the middle: a program that opens a relationship with a holder over DIDComm, issues
//! against a template through the holder's mediator, collects what the holder decided, revokes,
//! and serves a request that a wallet answers with a presentation — every step against a real
//! node, pinned to that node's own key.
//!
//! It exists so that the rules a node applies are seen to work from the other side of the wire,
//! by a program that shares no screen with the holder's app and no process with the node. What it
//! seals is what a wallet opens; what it asks is what a wallet is shown; what it concludes is what
//! `almena_sdk::verifier` concludes.
//!
//! # What it deliberately does not do
//!
//! **Nothing is invented to fill a step.** Attributes come from the command line or a file an
//! operator wrote; keys come from a directory the operator named; the template, the issuer's key,
//! the status list and the holder's own identifier all come from the record, read through a node.
//! And nothing is believed for having arrived: every act is fetched by the name it was asked for,
//! every chain is walked back to a creation that names itself, and every envelope is opened with
//! the relationship's own key before its sender is trusted.
//!
//! # Where the holder's app is replicated, and why it is not imported
//!
//! The envelope, the peer identifier and the mediator's asking are written here a second time, the
//! way the holder's app writes them, and held to the same published vectors. The two repositories
//! share no code on purpose: a format both import is a format one of them owns, and a disagreement
//! between two honest copies is a failing test rather than a version somebody has to bump.

pub mod answer;
pub mod chain;
pub mod commands;
pub mod directory;
pub mod failed;
pub mod issued;
pub mod link;
pub mod lists;
pub mod node;
pub mod post;
pub mod records;
pub mod relations;
pub mod verifying;
