//! Bringing a node up, and what it can honestly say about itself while it is up.
//!
//! There is no peer-to-peer layer in this repository, so a node started here joins nothing.
//! That is a fact this module reports rather than one it hides: every figure below is an
//! `Option` and every one of them is `None`, because nothing has been measured — not because
//! something was measured and came back empty.
//!
//! `.agents/rules/honest-emptiness.md` is the rule, and `null` is not zero is the half of it
//! that lives in these types.

use std::path::{Path, PathBuf};

use log::info;

use crate::IDENTIFIER;

/// A node, running.
///
/// Holding one of these means the node is up. Dropping it is not how it is stopped — see
/// [`Node::stop`], which says so in the record.
#[derive(Debug)]
pub struct Node {
    /// Where this node keeps things, resolved once at start.
    directories: almena_paths::Paths,
    /// The file its records are going to, when they are going to one at all.
    records: Option<PathBuf>,
}

impl Node {
    /// Brings the node up.
    ///
    /// `records` is the file this node's records are being written to, or `None` when they
    /// are only reaching the terminal. It is taken rather than discovered because installing
    /// the destination happens before there is a node to install it for.
    #[must_use]
    pub fn start(records: Option<PathBuf>) -> Self {
        info!("node_started identifier={IDENTIFIER}");

        Self {
            directories: almena_paths::Paths::for_application(IDENTIFIER),
            records,
        }
    }

    /// Which network this node belongs to.
    ///
    /// `None` because nobody has looked: belonging to a network means reading that network's
    /// configuration, and nothing here reads one yet. It is not "no network" — that is a
    /// different fact and it would deserve a different answer.
    #[must_use]
    pub fn network(&self) -> Option<&str> {
        None
    }

    /// Who this node is.
    ///
    /// `None` because it is nobody yet. A node is identified by a key generated on its own
    /// device, and which kind of key belongs to the peer-to-peer layer, which is not written.
    /// There is now somewhere to keep one — [`Self::application_data`] — and nothing to put
    /// there.
    #[must_use]
    pub fn identity(&self) -> Option<&str> {
        None
    }

    /// How many peers this node is talking to.
    ///
    /// `None` and never `0`. Zero would be a count somebody took; this is the absence of one.
    #[must_use]
    pub fn peers(&self) -> Option<usize> {
        None
    }

    /// Where this node would keep what it cannot get back.
    ///
    /// # Errors
    ///
    /// [`almena_paths::NoHomeDirectory`] when the platform does not say where the user's home
    /// is, in which case this node can store nothing at all.
    pub fn application_data(&self) -> Result<PathBuf, almena_paths::NoHomeDirectory> {
        self.directories.application_data()
    }

    /// The file this node's records are going to, if any.
    ///
    /// `None` means they are reaching the terminal and nothing else, which is what a node with
    /// no writable directory gets.
    #[must_use]
    pub fn records(&self) -> Option<&Path> {
        self.records.as_deref()
    }

    /// Takes the node down, saying so.
    pub fn stop(self) {
        info!("node_stopped");
    }
}

#[cfg(test)]
mod tests {
    use super::Node;

    #[test]
    fn a_new_node_has_measured_nothing() {
        let node = Node::start(None);

        // Each of these is `None` rather than an empty string or a zero, and this test is what
        // would fail if somebody made one of them "friendlier" by giving it a default.
        assert!(node.network().is_none());
        assert!(node.identity().is_none());
        assert!(node.peers().is_none());
    }

    #[test]
    fn a_node_knows_where_it_would_keep_things() {
        let node = Node::start(None);
        let directory = node.application_data().expect("a home directory");
        assert!(
            directory.to_string_lossy().contains("network.almena.cli"),
            "{directory:?}"
        );
    }
}
