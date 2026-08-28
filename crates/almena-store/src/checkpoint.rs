//! A summary of an object's state that falls over if it leaves anything out.
//!
//! An object's state is not written anywhere: it is worked out by replaying its chain from the
//! start. That is fine for an account with a dozen acts and ruinous for a network where everybody
//! arriving redoes everybody's history. A checkpoint is the state so far, signed inside the
//! object's own chain, so that whoever comes later takes **the checkpoint and what came after it**.
//!
//! # Signing it stops anybody else forging it, and does not stop the object lying
//!
//! Only the object can sign its own summary, so no node, competitor or platform can make one up.
//! But an object — or somebody holding one of its keys — can sign a summary that leaves something
//! out, and a routine signature would then be making claims about governance to everybody who
//! arrives afterwards.
//!
//! That is closed without raising any threshold and without asking anybody for anything:
//!
//! > **Every field carries the hash of the act that last set it.** *The devices are these, and
//! > `h1` set them.*
//!
//! # Why that is enough, and cheap
//!
//! The log carries, for every entry, **which object it is about and what kind of act it is** — and
//! every node holds the log. So anybody can look at that object's entries and ask whether some act
//! that governs this field came *after* the one the checkpoint cites. No history, nobody asked, and
//! no trust in whoever served the checkpoint.
//!
//! **A checkpoint that leaves out a governing act falls over on the first look.** It stays signed
//! and demonstrable for ever, and nobody has to wait for somebody to notice.
//!
//! # What governs what is protocol, not convention
//!
//! Which kinds of act govern which fields comes from the table of operations and is versioned with
//! it. Guessing it here would make a checkpoint fall over or stand up depending on who was reading.

use almena_format::entry::Entry;
use almena_format::identifier::Name;

use crate::kind::Kind;

/// A part of an object's state, and which act last set it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Claim {
    /// Which part of the state this is about.
    pub about: Governs,
    /// The act that last set it, as this checkpoint claims.
    pub set_by: Name,
}

/// A part of an object's state that some kinds of act govern.
///
/// **Not every field of every object** — only the ones a checkpoint may claim, which is the same
/// list as the ones an act can change. What an entity's checkpoint may claim arrives with entities.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Governs {
    /// The key that controls an account.
    Control,
    /// The devices an account may act through.
    Devices,
}

impl Governs {
    /// The kinds of act that set this part of the state.
    ///
    /// From the table of operations, which is what makes a checkpoint fall over or stand up the
    /// same way for everybody reading it.
    #[must_use]
    pub fn set_by(self) -> &'static [Kind] {
        match self {
            // Creating an account establishes its control key; rotating and recovering replace it.
            Self::Control => &[
                Kind::HOLDER_CREATE,
                Kind::HOLDER_ROTATE,
                Kind::HOLDER_RECOVER,
            ],
            Self::Devices => &[Kind::HOLDER_ADD_DEVICE, Kind::HOLDER_REMOVE_DEVICE],
        }
    }
}

/// The act a checkpoint left out, if it left one out.
///
/// `entries` is what the log holds about that object, in the order it wrote them — which every node
/// has, and which is the whole reason this can be checked by anybody without asking anybody.
///
/// [`None`] means nothing governing that field happened after the act it cites, so the claim is as
/// good as the log can say. It is not a promise the checkpoint is honest about the *value* — only
/// that it is not hiding a later act, which is the part a signature could otherwise be used to
/// paper over.
#[must_use]
pub fn left_out(claim: &Claim, entries: &[&Entry]) -> Option<Name> {
    let governs = claim.about.set_by();

    // Everything after the act it cites. An act it does not know about at all is the interesting
    // case: a checkpoint citing something that is not in this object's chain has cited nothing.
    let after = entries
        .iter()
        .position(|entry| entry.hash == claim.set_by)
        .map_or(0, |at| at + 1);

    entries
        .iter()
        .skip(after)
        .find(|entry| Kind::new(entry.kind).is_some_and(|kind| governs.contains(&kind)))
        .map(|entry| entry.hash.clone())
}

/// Everything a checkpoint left out.
///
/// **All of them, not the first.** A summary that hid two acts is a different thing from one that
/// hid one, and whoever is looking at it should see the whole of what it left out.
#[must_use]
pub fn falls_over(claims: &[Claim], entries: &[&Entry]) -> Vec<(Governs, Name)> {
    claims
        .iter()
        .filter_map(|claim| left_out(claim, entries).map(|missed| (claim.about, missed)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{Claim, Governs, falls_over, left_out};
    use almena_format::cbor::Value;
    use almena_format::entry::Entry;
    use almena_format::identifier::{Name, Network};
    use almena_format::operation::{Operation, create};
    use almena_time::Epoch;
    use std::collections::BTreeMap;

    /// An act of that kind about that object, at that position.
    fn act(kind: crate::kind::Kind, at: u64) -> (Operation, Entry) {
        let operation = create(
            Network::Development,
            kind.number(),
            1,
            Epoch::GENESIS,
            BTreeMap::from([(1, Value::Uint(at))]),
        );
        let entry = Entry::of(&operation, at, None);
        (operation, entry)
    }

    /// An account's chain: created, a device added, then whatever else is asked for.
    fn a_chain(after: &[crate::kind::Kind]) -> Vec<Entry> {
        let mut entries = vec![act(crate::kind::Kind::HOLDER_CREATE, 0).1];
        entries.push(act(crate::kind::Kind::HOLDER_ADD_DEVICE, 1).1);
        for (which, kind) in after.iter().enumerate() {
            entries.push(act(*kind, 2 + which as u64).1);
        }
        entries
    }

    #[test]
    fn a_claim_with_nothing_after_it_stands() {
        let entries = a_chain(&[]);
        let held: Vec<&Entry> = entries.iter().collect();

        assert_eq!(
            left_out(
                &Claim {
                    about: Governs::Devices,
                    set_by: entries[1].hash.clone(),
                },
                &held
            ),
            None
        );
    }

    #[test]
    fn a_claim_that_hides_a_later_act_falls_over() {
        // **The whole point.** A summary signed with a routine key would otherwise make claims
        // about what an account is, to everybody arriving afterwards.
        let entries = a_chain(&[crate::kind::Kind::HOLDER_ADD_DEVICE]);
        let held: Vec<&Entry> = entries.iter().collect();

        assert_eq!(
            left_out(
                &Claim {
                    about: Governs::Devices,
                    set_by: entries[1].hash.clone(),
                },
                &held
            ),
            Some(entries[2].hash.clone()),
            "and it says which act it left out"
        );
    }

    #[test]
    fn an_act_that_governs_something_else_does_not_make_it_fall_over() {
        // A device added after the control key was set says nothing about the control key. Reading
        // it as though it did would make honest summaries fall over.
        let entries = a_chain(&[crate::kind::Kind::HOLDER_ADD_DEVICE]);
        let held: Vec<&Entry> = entries.iter().collect();

        assert_eq!(
            left_out(
                &Claim {
                    about: Governs::Control,
                    set_by: entries[0].hash.clone(),
                },
                &held
            ),
            None
        );
    }

    #[test]
    fn rotating_makes_a_claim_about_the_control_key_fall_over() {
        let entries = a_chain(&[crate::kind::Kind::HOLDER_ROTATE]);
        let held: Vec<&Entry> = entries.iter().collect();

        assert_eq!(
            left_out(
                &Claim {
                    about: Governs::Control,
                    set_by: entries[0].hash.clone(),
                },
                &held
            ),
            Some(entries[2].hash.clone())
        );
    }

    #[test]
    fn a_claim_citing_something_not_in_this_chain_has_cited_nothing() {
        // Otherwise a summary could point at an act nobody can find and be treated as current.
        let entries = a_chain(&[]);
        let held: Vec<&Entry> = entries.iter().collect();

        assert_eq!(
            left_out(
                &Claim {
                    about: Governs::Devices,
                    set_by: Name::of(b"an act of somebody else's"),
                },
                &held
            ),
            Some(entries[1].hash.clone()),
            "everything that governs it is later than nothing"
        );
    }

    #[test]
    fn everything_left_out_is_said_and_not_only_the_first() {
        // A summary that hid two things is a different thing from one that hid one.
        let entries = a_chain(&[
            crate::kind::Kind::HOLDER_ADD_DEVICE,
            crate::kind::Kind::HOLDER_ROTATE,
        ]);
        let held: Vec<&Entry> = entries.iter().collect();

        let fell = falls_over(
            &[
                Claim {
                    about: Governs::Devices,
                    set_by: entries[1].hash.clone(),
                },
                Claim {
                    about: Governs::Control,
                    set_by: entries[0].hash.clone(),
                },
            ],
            &held,
        );
        assert_eq!(fell.len(), 2);
    }

    #[test]
    fn a_summary_that_leaves_nothing_out_does_not_fall_over() {
        let entries = a_chain(&[crate::kind::Kind::HOLDER_ADD_DEVICE]);
        let held: Vec<&Entry> = entries.iter().collect();

        assert!(
            falls_over(
                &[Claim {
                    about: Governs::Devices,
                    set_by: entries[2].hash.clone(),
                }],
                &held
            )
            .is_empty(),
            "citing the latest act that governs it is exactly what an honest one does"
        );
    }
}
