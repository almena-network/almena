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
//! A capability neither face offers is allowed, and saying so costs a line. A capability one face
//! offers and the other does not is allowed **only with the reason written beside it**: *not yet*
//! is a claim somebody has to be willing to put in words, and a row that reads badly a year on is
//! the point. What is not allowed is a capability quietly missing from the table, which is why
//! every one the core can do has a row — and why each face carries a test of its own that maps
//! its real surface, the flags it parses or the commands it registers, onto its column here.

/// Something a person can do to a node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Capability {
    /// Open a network, when there is nobody to join.
    OpenNetwork,
    /// Join the network the zone names, by asking somebody already on it for the record.
    JoinNetwork,
    /// Come back to the network the directory already holds, without being asked.
    ComeBack,
    /// Say whether this build's format is one a network may be opened on for good.
    FreezeChecklist,
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
    /// Serve the interface under a certificate an operator names, instead of the node's own key.
    Certificate,
    /// Take a place on the mesh, so other nodes can reach this one.
    JoinTheMesh,
    /// Hold post for people whose device is not on: the mailbox, said in the record.
    Mediator,
    /// Choose the language it speaks, and have it remembered.
    Language,
    /// Say who contributed this node, and stop saying it.
    ///
    /// Both halves are one capability because a face that could bind and not let go would be a face
    /// that writes somebody's name into a record and cannot take it back.
    SayWhoContributedIt,
    /// Close this node for good, so that it stops counting.
    CloseThisNode,
    /// Say what a zone would have to carry for this node to be a seed.
    ///
    /// Not publishing — nothing here writes to a zone, and nothing asks anybody to. It is the node
    /// saying the parts of that record only it can produce, so that an operator asking to be a seed
    /// hands over something correct instead of assembling it by hand.
    SayHowToFindMe,
    /// Erase this node from the machine it is on: tell the network, then take the directory away.
    ///
    /// Not a second way of closing. Closing is what a node says to everybody else, and it is said
    /// for ever; this is what happens to the files afterwards, and it is the only way back to a
    /// machine that is not a node — which is a thing somebody has to be able to reach when the
    /// node will not come up at all, because a way out that needs a working node is not one.
    EraseThisNode,
    /// Be the node in a directory somebody named, instead of the usual one.
    Directory,
    /// Say where to look for the network: another zone, a seed by hand, a resolver by address.
    ///
    /// One capability and not three, because they are one question — *how does this node find
    /// out who is there* — asked three ways.
    WhereToLook,
    /// Open a development network without asking the zone, on somebody's word that nobody is there.
    NobodyIsThere,
    /// Publish the core Almena maintains, as Almena Government.
    PublishCore,
    /// Certify an entity, as Almena Government.
    Certify,
    /// Answer an asking to be certified, as Almena Government.
    Reply,
    /// Move the clock forward by the epochs written in a file, on the development network alone.
    ///
    /// A device the words add waits three days before it may sign, and a network opened this
    /// morning has to be walkable this morning — so a development node reads an offset from a
    /// file on every look at its clock, and the file is what a test moves. Production never
    /// reads one: its clock is the wall's and nothing on a command line or in an environment
    /// changes that.
    ClockOffset,
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
pub const FACES: [Offered; 24] = [
    Offered {
        // Both faces, on the zone's word that there is nobody to join. Development is opened as
        // often as it needs to be; production is opened once, ever, after the freeze checklist,
        // and it is the same act asked of a different zone.
        // **Both, and the window opens development alone.** Its one press joins the network the
        // build is for, and falls through to opening only where that is development — which is
        // opened as often as it needs to be. Production is refused on the argument itself, before
        // anything happens, so there is no ordering of events that reaches a production network
        // being opened from a window: that is the accident §4.5 calls the one that costs the most,
        // and it is a deliberate act at a terminal.
        capability: Capability::OpenNetwork,
        window: true,
        terminal: true,
        not_yet: "",
    },
    Offered {
        capability: Capability::JoinNetwork,
        window: true,
        terminal: true,
        not_yet: "",
    },
    Offered {
        // Every start after the first, in both: the window on launch, the terminal with no flag.
        capability: Capability::ComeBack,
        window: true,
        terminal: true,
        not_yet: "",
    },
    Offered {
        // It was shown under the production card on the screen that chose a network, so that a
        // refusal to open one was never the first anybody heard of it. There is no such screen
        // and no such press, so there is nothing for it to warn about.
        capability: Capability::FreezeChecklist,
        window: false,
        terminal: true,
        not_yet: "nothing in the window opens a production network, so there is no refusal to warn about",
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
        // The node closes its own epochs on a timer from the moment it is on a network. The
        // window had a button for not waiting for it, which is a thing to want while writing
        // the software and not while running a node.
        capability: Capability::CloseEpoch,
        window: false,
        terminal: true,
        not_yet: "the node closes its own epochs on a timer; not waiting for it is a developer's want",
    },
    Offered {
        capability: Capability::JoinTheMesh,
        window: true,
        terminal: true,
        not_yet: "",
    },
    Offered {
        // Both faces say so in the record too: `Interface` goes on the node's own chain the moment
        // the socket is bound, once, the way the mediator flag says `Mailbox` — a node answering on
        // an interface it never announced would be one the network's figures could not count.
        capability: Capability::Serve,
        window: true,
        terminal: true,
        not_yet: "",
    },
    Offered {
        // A node serves under its own key, always, and the window never asks about that: a pair
        // of files is for an operator who already has a certificate from an authority.
        capability: Capability::Certificate,
        window: false,
        terminal: true,
        not_yet: "the window is a node that starts by itself; operating one by hand is the terminal's",
    },
    Offered {
        // Holding post for other people is a capacity somebody takes on deliberately, and it is
        // said in the record. The window takes its mesh place with it off.
        capability: Capability::Mediator,
        window: false,
        terminal: true,
        not_yet: "the window is a node that starts by itself; operating one by hand is the terminal's",
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
        // **This one costs something and it is worth saying plainly.** A node bound to whoever
        // contributed it is how that person earns write credit (§4.7), and the window is where
        // most people will run a node — so a window that cannot show the challenge is a window
        // whose nodes go uncredited unless their operator also has a terminal.
        capability: Capability::SayWhoContributedIt,
        window: false,
        terminal: true,
        not_yet: "the window shows no challenge, so a node run from it goes uncredited without a terminal",
    },
    Offered {
        // Both faces. Whoever keeps the zone gets one record whichever face the operator runs, and
        // a face that composed its own would be a second implementation of a format that is
        // verified against — a `_seed` with the wrong `net=` points at a network that is not this
        // one. So the node composes it and the faces only carry it.
        capability: Capability::SayHowToFindMe,
        window: true,
        terminal: true,
        not_yet: "",
    },
    Offered {
        // Erasing this node is in the window and says the close for it, so the *act* still
        // happens from there; what the window no longer has is closing a node and keeping it on
        // disk, which is an operator taking a node out of the census to look at afterwards.
        capability: Capability::CloseThisNode,
        window: false,
        terminal: true,
        not_yet: "the window closes a node only by erasing it; closing one and keeping it is an operator's",
    },
    Offered {
        // Both faces, and neither of them may refuse it over a node that is down: the machine
        // this is asked on is the one somebody is holding, and the state it has to be able to
        // reach — no longer a node — cannot be made to depend on the node working.
        capability: Capability::EraseThisNode,
        window: true,
        terminal: true,
        not_yet: "",
    },
    Offered {
        // The window keeps one directory per network under the platform's application data, and
        // that is the whole of its answer: a person who wants two windowed nodes on one machine
        // wants two installations, which is not a thing this application offers. The terminal
        // is where a machine runs several nodes, and it names each directory.
        capability: Capability::Directory,
        window: false,
        terminal: true,
        not_yet: "the window keeps one directory per network; naming another is a terminal thing",
    },
    Offered {
        // The terminal takes a zone, seeds and resolvers as flags; the window takes a zone and a
        // pasted seed behind a disclosure on the screen that chooses the network, and reads the
        // resolver from `ALMENA_RESOLVER` while developing. Nothing a deployment sets.
        // The window asks the network's own zone and nothing else. A zone that is not the
        // network's, a seed by hand and a resolver by address are for a network being tried out
        // on one machine, which is what the terminal is for.
        capability: Capability::WhereToLook,
        window: false,
        terminal: true,
        not_yet: "the window is a node that starts by itself; operating one by hand is the terminal's",
    },
    Offered {
        // It only ever reached development, and only from the screen that chose a network.
        capability: Capability::NobodyIsThere,
        window: false,
        terminal: true,
        not_yet: "the window is a node that starts by itself; operating one by hand is the terminal's",
    },
    Offered {
        // A government ceremony is a terminal act in this version: it runs against the directory
        // of the node that opened the network, once, by whoever holds that machine — and a button
        // for it in a window would be a button that publishes the core of the whole network from
        // wherever the window happened to be open.
        capability: Capability::PublishCore,
        window: false,
        terminal: true,
        not_yet: "a government ceremony is a terminal act in this version",
    },
    Offered {
        capability: Capability::Certify,
        window: false,
        terminal: true,
        not_yet: "a government ceremony is a terminal act in this version",
    },
    Offered {
        capability: Capability::Reply,
        window: false,
        terminal: true,
        not_yet: "a government ceremony is a terminal act in this version",
    },
    Offered {
        // The terminal takes the file as `--clock-offset-file`, refused beside the production
        // network before anything starts; the window reads `ALMENA_CLOCK_OFFSET_FILE` from the
        // environment while developing and ignores it on production. Both re-read the file on
        // every look at the clock and say in the records when the number changes. Nothing a
        // deployment sets.
        capability: Capability::ClockOffset,
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
    fn neither_face_can_do_something_the_other_cannot_without_saying_why() {
        // The whole rule, and the only one that has to hold every day. A feature that reached one
        // face and not the other fails here rather than being discovered by somebody who went
        // looking for it on the machine that did not have it — unless the table says, in words,
        // why one face is without it. Silence is what this refuses; a reason is what it demands.
        for offered in FACES {
            assert!(
                offered.window == offered.terminal || !offered.not_yet.is_empty(),
                "{:?} is offered by one face and not the other, and nothing says why",
                offered.capability
            );
        }
        // And the window never has something the terminal lacks: a machine with no screen is
        // the one that must not fall behind, because nobody is looking at it.
        for capability in offered_by_window() {
            assert!(
                offered_by_terminal().contains(&capability),
                "{capability:?} reached the window and not the terminal"
            );
        }
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
            Capability::JoinNetwork,
            Capability::ComeBack,
            Capability::FreezeChecklist,
            Capability::Watch,
            Capability::Resolve,
            Capability::Deliver,
            Capability::CloseEpoch,
            Capability::Serve,
            Capability::Certificate,
            Capability::JoinTheMesh,
            Capability::Mediator,
            Capability::SayWhoContributedIt,
            Capability::SayHowToFindMe,
            Capability::CloseThisNode,
            Capability::EraseThisNode,
            Capability::Directory,
            Capability::WhereToLook,
            Capability::NobodyIsThere,
            Capability::PublishCore,
            Capability::Certify,
            Capability::Reply,
            Capability::Language,
            Capability::ClockOffset,
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
