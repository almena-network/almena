//! The six reserved extension holes, written down here so that code cannot forget one.
//!
//! A hole is a decision that is not implemented today but whose **format has to allow for it**, so
//! that the day it arrives it is an addition and not a migration. Everything written to a
//! production log is there for ever, and nothing already written may be reinterpreted.
//!
//! # What is reserved here is not a number
//!
//! A payload is a sparse map of integer keys. A key nobody uses today costs nothing and
//! can be handed out the day it is needed, so reserving numbers in advance buys nothing — and
//! costs something, because it would fix the shape of fields whose content is not designed yet.
//! **What is fixed is the table**: which carrier each hole lives in, and which of the three
//! mechanisms keeps an old reader honest about it.
//!
//! # Three mechanisms, not one
//!
//! All six holes were once thought to depend on the criticality mark. They do not, and
//! believing it leads to reserving numbers for holes that are not fields while leaving the ones
//! that are without protection:
//!
//! | Mechanism | Protects | How an old reader fails |
//! |---|---|---|
//! | [`Protection::Criticality`] | A **new field** in an existing type | Does not know the odd number, declares the operation unintelligible |
//! | [`Protection::UnknownType`] | A **whole new operation** | Does not know the `tipo`, replicates anyway, declares that object unresolvable |
//! | [`Protection::ClosedVocabulary`] | A **new value** in a field that already existed | Does not know the value, refuses instead of taking it for the default |

/// Where a hole actually lives, which decides what can protect it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Carrier {
    /// Inside the `payload` of an operation type that already exists.
    Payload,
    /// A new operation type of its own. The log entry's schema does not change.
    OperationType,
    /// Outside the log altogether — the epoch roots are not entries.
    OutsideTheLog,
    /// The credential, which is an SD-JWT VC and not a registry object at all.
    Credential,
}

/// What keeps an old reader from misreading a hole once it is used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protection {
    /// An odd field number: unknown means unintelligible.
    Criticality,
    /// An unknown `tipo`: replicate, and declare the object unresolvable.
    UnknownType,
    /// A fixed list of values: one outside it is refused rather than mistaken for a neighbour.
    ClosedVocabulary,
    /// Nothing has to protect it, because ignoring it cannot produce a false statement.
    HarmlessToIgnore,
}

/// Whether the hole's carrier is already in the format or arrives with the feature.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ships {
    /// The field is in every operation from day one; what grows later is its vocabulary.
    Now,
    /// Nothing is written until the feature exists.
    WhenUsed,
}

/// One reserved hole.
#[derive(Debug, Clone, Copy)]
pub struct Hole {
    /// The name it was reserved under.
    pub name: &'static str,
    /// Where it lives.
    pub carrier: Carrier,
    /// What keeps a reader honest about it.
    pub protection: &'static [Protection],
    /// Whether anything is written today.
    pub ships: Ships,
}

/// All six of them.
pub const HOLES: [Hole; 6] = [
    Hole {
        name: "alcance del sello",
        carrier: Carrier::Payload,
        protection: &[Protection::Criticality],
        ships: Ships::WhenUsed,
    },
    Hole {
        // The annotation is an object with its own chain and a `sujeto`, so the entry schema
        // does not change — which is what makes a move to facts-plus-annotations an addition.
        name: "referencias entre entradas del log",
        carrier: Carrier::OperationType,
        protection: &[Protection::UnknownType],
        ships: Ships::WhenUsed,
    },
    Hole {
        // Ignoring an anchor lowers the firmness a reader counts and never makes it say anything
        // false: firmness is counted in independent trees, so a missing anchor is a floor.
        name: "anclaje externo de raíces",
        carrier: Carrier::OutsideTheLog,
        protection: &[Protection::HarmlessToIgnore],
        ships: Ships::WhenUsed,
    },
    Hole {
        name: "tipo de prueba en credenciales",
        carrier: Carrier::Credential,
        protection: &[Protection::ClosedVocabulary],
        ships: Ships::Now,
    },
    Hole {
        name: "método de identificación del emisor",
        carrier: Carrier::Credential,
        protection: &[Protection::ClosedVocabulary],
        ships: Ships::Now,
    },
    Hole {
        name: "método de una propuesta",
        carrier: Carrier::Payload,
        protection: &[Protection::Criticality, Protection::ClosedVocabulary],
        ships: Ships::Now,
    },
];

#[cfg(test)]
mod tests {
    use super::{Carrier, HOLES, Hole, Protection, Ships};

    #[test]
    fn there_are_six_of_them() {
        // Six were reserved. A seventh invented here, or one of the six quietly dropped, is a
        // format that has stopped matching the decisions it exists to keep room for.
        assert_eq!(HOLES.len(), 6);
    }

    #[test]
    fn criticality_is_only_claimed_where_it_could_possibly_work() {
        // The error this table itself used to contain. The mark is a field number, so only a
        // hole living in a payload can be protected by one — a new operation type has no field to
        // mark, and neither a root nor an SD-JWT VC is an integer-keyed map of ours at all.
        for hole in HOLES {
            if hole.protection.contains(&Protection::Criticality) {
                assert_eq!(hole.carrier, Carrier::Payload, "{}", hole.name);
            }
        }
    }

    #[test]
    fn everything_that_ships_now_has_a_closed_vocabulary() {
        // A field present on day one is known to every reader, so its parity never fires and what
        // grows is the vocabulary. Without a closed set it would be extended silently.
        for hole in HOLES {
            if hole.ships == Ships::Now {
                assert!(
                    hole.protection.contains(&Protection::ClosedVocabulary),
                    "{} ships now and would be extended by value",
                    hole.name
                );
            }
        }
    }

    #[test]
    fn nothing_is_left_unprotected() {
        for hole in HOLES {
            assert!(!hole.protection.is_empty(), "{}", hole.name);
        }
    }

    #[test]
    fn only_one_may_be_ignored_with_impunity() {
        // And it is the one outside the log. Anywhere else, "harmless to ignore" would be a claim
        // that a reader can skip something and still be telling the truth — serving the previous
        // state as if it were current, which is the one thing a node may never do.
        let harmless: Vec<&Hole> = HOLES
            .iter()
            .filter(|hole| hole.protection.contains(&Protection::HarmlessToIgnore))
            .collect();
        assert_eq!(harmless.len(), 1);
        assert_eq!(harmless[0].carrier, Carrier::OutsideTheLog);
    }

    #[test]
    fn the_two_credential_holes_are_not_this_format() {
        // Worth asserting because it is the assumption that would otherwise be made: an SD-JWT VC
        // has string claim names, so parity is not available there and the SDK will own them.
        let credential = HOLES.iter().filter(|h| h.carrier == Carrier::Credential);
        assert_eq!(credential.count(), 2);
    }
}
