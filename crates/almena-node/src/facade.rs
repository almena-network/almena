//! Everything a person can do to a node, and which of its two faces can do it.
//!
//! A node runs one of two ways — in a window, or in a terminal — and **neither is the other with
//! things taken out**. Everything possible in one is possible in the other. That is easy to write
//! down and hard to keep: what actually happens is that a feature reaches the face somebody was
//! looking at, the other quietly falls behind, and nobody notices until somebody tries.
//!
//! So it is not left to whoever remembers. This table is what both faces are held to, and the
//! checks below fail the build when they stop matching.
//!
//! # Why the table is here and not in either face
//!
//! Because neither face may see the other. A terminal node must not link a webview it never draws
//! in, and a windowed one must not link a terminal renderer — the dependency graph refuses both.
//! So a check that compared the two would have nowhere to live. Declaring here, where both already
//! look, is what makes the comparison possible at all.
//!
//! # Not yet drawn is a state, and it has to be written down
//!
//! A capability neither face offers is allowed, and saying so costs a line. What is not allowed is
//! one face offering something and the other not — and what is not allowed either is a capability
//! quietly missing from the table, which is why every one the core can do has a row.

/// Something a person can do to a node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Capability {
    /// Open a network, when there is nobody to join.
    OpenNetwork,
    /// See what this node is: its network, what it has written down, where it is up to.
    Watch,
    /// Ask it about an object.
    Resolve,
    /// Hand it a signed act.
    Deliver,
    /// Close an epoch and publish the root, whether or not anything happened.
    CloseEpoch,
    /// Turn the interface on, so that clients and portals can ask.
    Serve,
    /// Take a place on the mesh, so other nodes can reach this one.
    JoinTheMesh,
    /// Choose the language it speaks, and have it remembered.
    Language,
    /// Say who contributed this node, and stop saying it.
    ///
    /// Both halves are one capability because a face that could bind and not let go would be a face
    /// that writes somebody's name into a record and cannot take it back.
    SayWhoContributedIt,
}

/// One capability, and which faces have actually drawn it.
#[derive(Debug, Clone, Copy)]
pub struct Offered {
    /// What it is.
    pub capability: Capability,
    /// Whether the windowed face offers it.
    pub window: bool,
    /// Whether the terminal face offers it.
    pub terminal: bool,
    /// Why neither does, when neither does. Empty when both do.
    ///
    /// It is a sentence rather than a flag on purpose: *not yet* is a claim somebody has to be
    /// willing to write down, and one that reads badly next to a capability that has been sitting
    /// there for a year.
    pub not_yet: &'static str,
}

/// Every capability, and where each one stands.
pub const FACES: [Offered; 9] = [
    Offered {
        // Only development, in both faces, and on somebody's word that there is nobody to join —
        // because nothing reads a zone yet. A production network is opened once, ever, and neither
        // face offers a way to do it on a promise.
        capability: Capability::OpenNetwork,
        window: true,
        terminal: true,
        not_yet: "",
    },
    Offered {
        capability: Capability::Watch,
        window: true,
        terminal: true,
        not_yet: "",
    },
    Offered {
        capability: Capability::Resolve,
        window: false,
        terminal: false,
        not_yet: "asking a node about an object is worth drawing once a node holds anything",
    },
    Offered {
        capability: Capability::Deliver,
        window: false,
        terminal: false,
        not_yet: "acts arrive over the interface; a face offering it too waits until one does",
    },
    Offered {
        capability: Capability::CloseEpoch,
        window: true,
        terminal: true,
        not_yet: "",
    },
    Offered {
        capability: Capability::JoinTheMesh,
        window: true,
        terminal: true,
        not_yet: "",
    },
    Offered {
        capability: Capability::Serve,
        window: true,
        terminal: true,
        not_yet: "",
    },
    Offered {
        capability: Capability::Language,
        window: true,
        terminal: true,
        not_yet: "",
    },
    Offered {
        // The node shows a challenge and whoever is claiming it approves it with the key their own
        // chain authorises. Both faces show and both faces record, because an operator who could
        // only do it from a terminal would be an operator who has to have one.
        capability: Capability::SayWhoContributedIt,
        window: true,
        terminal: true,
        not_yet: "",
    },
];

/// What one face offers, for that face's own check.
///
/// Each face asserts against this that what it draws is what the table says it draws. Neither can
/// check the other — nothing links both — so the table is the only place the two meet, and each
/// face is answerable for its own column.
#[must_use]
pub fn offered_by_window() -> Vec<Capability> {
    FACES
        .iter()
        .filter(|offered| offered.window)
        .map(|offered| offered.capability)
        .collect()
}

/// The same, for the terminal.
#[must_use]
pub fn offered_by_terminal() -> Vec<Capability> {
    FACES
        .iter()
        .filter(|offered| offered.terminal)
        .map(|offered| offered.capability)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{Capability, FACES, offered_by_terminal, offered_by_window};

    #[test]
    fn neither_face_can_do_something_the_other_cannot() {
        // The whole rule, and the only one that has to hold every day. A feature that reached one
        // face and not the other fails here rather than being discovered by somebody who went
        // looking for it on the machine that did not have it.
        for offered in FACES {
            assert_eq!(
                offered.window, offered.terminal,
                "{:?} is offered by one face and not the other",
                offered.capability
            );
        }
        assert_eq!(offered_by_window(), offered_by_terminal());
    }

    #[test]
    fn a_capability_nobody_draws_yet_has_to_say_so_in_words() {
        // *Not yet* is allowed and silence is not. A blank here would let a capability sit
        // undrawn indefinitely with nothing to read that says it is.
        for offered in FACES {
            let drawn = offered.window && offered.terminal;
            assert_eq!(
                drawn,
                offered.not_yet.is_empty(),
                "{:?} is either drawn by both faces or says why not — never neither",
                offered.capability
            );
        }
    }

    #[test]
    fn every_capability_appears_exactly_once() {
        // A row that went missing would take its capability out of the check silently, which is
        // the one way this table can fail without failing.
        let listed: Vec<Capability> = FACES.iter().map(|offered| offered.capability).collect();
        for capability in [
            Capability::OpenNetwork,
            Capability::Watch,
            Capability::Resolve,
            Capability::Deliver,
            Capability::CloseEpoch,
            Capability::Serve,
            Capability::SayWhoContributedIt,
            Capability::Language,
        ] {
            assert_eq!(
                listed.iter().filter(|&&it| it == capability).count(),
                1,
                "{capability:?}"
            );
        }
        assert_eq!(listed.len(), FACES.len());
    }
}
