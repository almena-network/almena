//! How Almena Government stands against what `SPECS.md §7.1` asks of it in production.
//!
//! On the day a network opens, its trust anchor is one self-signed key (`SPECS.md §7.9`). That is
//! the only way a web of trust can start, and there is nothing wrong with it — what would be wrong
//! is **leaving it that way without saying so**, because while Almena is a set of keys in one
//! person's hands, *the trust anchor of the network is a person*. `SPECS.md §7.1` says that in as
//! many words, and puts it beside `SPECS.md §7.7`'s single-owner warning as its close relative.
//!
//! So this is a reading, published, and never a verdict.
//!
//! # Why nothing here refuses anything
//!
//! `SPECS.md §7.1` is explicit that the composition is **configuration and not protocol**: the
//! specification fixes the mechanism — `k`-of-`n` and the three classes — and how many owners
//! Almena has at any moment is its own to change by governance, exactly as for every other
//! organisation. A node that refused Almena's certifications until it liked the numbers would be a
//! node deciding an organisation's governance for it, which is the thing this record does not do
//! anywhere else either: `SPECS.md §8.2` lets an organisation drop below its own threshold rather
//! than have a stranger's software stop it.
//!
//! What holds the line instead is that the numbers are **read from the record by anybody**, and
//! that the applications say what they read. A door that is open and in plain sight is a different
//! thing from a door that is open and unmentioned.
//!
//! # Three of the four can be read; the fourth is a declaration
//!
//! `n ≥ 5`, sealing `k ≥ 3` and governance `k > n/2` are in the record: they are owners and
//! thresholds and nothing else. **Where those owners are is not**, and cannot be — owners are root
//! identifiers and root identifiers are anonymous (`SPECS.md §8.1`), so no amount of reading tells
//! anybody whether five of them work at the same company.
//!
//! That is not a gap to paper over. The fourth criterion is met by **declaring** it and by that
//! declaration being checkable against itself, which is what [`spread`] does — and a reader who
//! wants more than a declaration has the same recourse they have everywhere else here: they choose
//! their own root of trust.

use std::collections::{BTreeMap, BTreeSet};

use almena_format::identifier::Did;

use crate::entity::Entity;

/// The fewest owners a production composition has.
///
/// **Five, and the reason is losing one.** `SPECS.md §7.1` puts the real failure not at starting
/// small but at *staying* small without noticing, and fixes the test accordingly: production does
/// not start with a composition that would not survive the loss of a key.
pub const OWNERS_AT_LEAST: u64 = 5;

/// The fewest owners a sealing act takes.
///
/// **Three, and it is the one of the four most worth arguing about.** Since `SPECS.md §9.4` the
/// seal is a *permission* and not only a signal — it is what lets an organisation publish the
/// shapes everybody else uses — so whatever number this is, compromising that many owners is enough
/// to let a hostile party define those shapes. `SPECS.md §8.7` gives the second argument, and it is
/// independent: Almena demands minimum owners and thresholds of whoever claims a high grade, and
/// demanding of others what it does not keep itself would not survive being noticed.
pub const SEALING_AT_LEAST: u64 = 3;

/// What is missing from a composition, one entry per thing.
///
/// Each carries the numbers rather than a sentence, so that whoever draws it can say it in the
/// reader's own language (`SPECS.md §13.9`) — the same rule the node's errors follow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Wanting {
    /// Fewer owners than a composition that survives losing one.
    TooFewOwners {
        /// How many there are.
        has: u64,
    },
    /// Sealing costs fewer owners than `SPECS.md §7.1` asks.
    SealingTooLow {
        /// What it costs.
        is: u64,
    },
    /// Governance is not more than half the owners, so half of them could change who governs.
    GovernanceIsNotAMajority {
        /// What it costs.
        is: u64,
        /// Out of how many owners.
        of: u64,
    },
    /// Half or more of the owners are in one place — one organisation, or one jurisdiction.
    ///
    /// **Five owners in one company are, in practice, one owner**, and the promise that Almena is
    /// not a central authority would rest on the goodwill of people who all know each other.
    TooManyInOne {
        /// The organisation or jurisdiction, as it was declared.
        called: String,
        /// How many owners are there.
        has: u64,
        /// Out of how many owners in all.
        of: u64,
    },
    /// Nobody is an owner yet, so the key the genesis gave it is what signs.
    ///
    /// **The starting state and not a fault** (`SPECS.md §7.9`), and the one that most has to be
    /// said out loud rather than left for somebody to work out.
    NobodyIsAnOwnerYet,
}

/// Where an owner is, as Almena declares it.
///
/// **A declaration, and it says so in its name.** Nothing in the record could carry this: an owner
/// is a root identifier and root identifiers are anonymous (`SPECS.md §8.1`), so where somebody
/// works is something Almena states about its own composition and not something a node checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Declared {
    /// The organisation the owner belongs to.
    pub organisation: String,
    /// The jurisdiction they are under.
    pub jurisdiction: String,
}

/// What the record alone says about the composition.
///
/// Three of `SPECS.md §7.1`'s four criteria, which are the three made of owners and thresholds.
/// Empty means those three are met — never that the composition is fit, which needs [`spread`] too.
#[must_use]
pub fn counted(body: &Entity) -> Vec<Wanting> {
    let owners = u64::try_from(body.owners.len()).unwrap_or(u64::MAX);
    if owners == 0 {
        return vec![Wanting::NobodyIsAnOwnerYet];
    }

    let mut wanting = Vec::new();
    if owners < OWNERS_AT_LEAST {
        wanting.push(Wanting::TooFewOwners { has: owners });
    }
    if body.thresholds.sealing < SEALING_AT_LEAST {
        wanting.push(Wanting::SealingTooLow {
            is: body.thresholds.sealing,
        });
    }
    // **Strictly more than half**, so that no half of the owners can change who the owners are.
    // Doubled rather than halved because owners are whole people: with five owners, half is two and
    // a half and the smallest majority is three, which integer division would have called two.
    if body.thresholds.governance * 2 <= owners {
        wanting.push(Wanting::GovernanceIsNotAMajority {
            is: body.thresholds.governance,
            of: owners,
        });
    }
    wanting
}

/// What the declaration says about the fourth criterion.
///
/// `where_they_are` is Almena's own statement about its owners. An owner it says nothing about is
/// counted **as its own place**, which is the generous reading and is deliberate: the alternative —
/// treating silence as *somewhere already counted* — would let a composition pass by saying less.
///
/// Both axes are checked, because `SPECS.md §7.1` names both and they fail differently: one company
/// holding half the seats is a commercial dependency, and one country holding half is a legal one.
#[must_use]
pub fn spread(owners: &BTreeSet<Did>, where_they_are: &BTreeMap<Did, Declared>) -> Vec<Wanting> {
    let all = u64::try_from(owners.len()).unwrap_or(u64::MAX);
    if all == 0 {
        return vec![Wanting::NobodyIsAnOwnerYet];
    }

    let mut counts: BTreeMap<String, u64> = BTreeMap::new();
    for owner in owners {
        let places = where_they_are.get(owner).map_or_else(
            || vec![owner.to_string()],
            |declared| vec![declared.organisation.clone(), declared.jurisdiction.clone()],
        );
        for place in places {
            *counts.entry(place).or_default() += 1;
        }
    }

    counts
        .into_iter()
        .filter(|(_, has)| has * 2 >= all)
        .map(|(called, has)| Wanting::TooManyInOne {
            called,
            has,
            of: all,
        })
        .collect()
}

/// Everything `SPECS.md §7.1` asks of a production composition, read and declared together.
///
/// Empty is the state in which certifying an outside organisation is not, by itself, one person
/// vouching for the network under another name.
#[must_use]
pub fn fit(body: &Entity, where_they_are: &BTreeMap<Did, Declared>) -> Vec<Wanting> {
    let mut wanting = counted(body);
    if wanting == vec![Wanting::NobodyIsAnOwnerYet] {
        return wanting;
    }
    wanting.extend(spread(&body.owners, where_they_are));
    wanting
}

/// Whether the trust anchor of this network is, in practice, whoever holds one key.
///
/// **The thing that has to be said** (`SPECS.md §7.1`). It is true from the genesis until the first
/// owner is named, and saying it is not an admission of a fault: it is the state of a project that
/// has started, and hiding it would be the fault.
#[must_use]
pub fn one_pair_of_hands(body: &Entity) -> bool {
    body.owners.is_empty()
}

#[cfg(test)]
mod tests {
    use super::{Declared, Wanting, counted, fit, one_pair_of_hands, spread};
    use crate::entity::{Entity, Thresholds, alone};
    use almena_format::identifier::{Did, Name, Network};
    use almena_time::Epoch;
    use std::collections::{BTreeMap, BTreeSet};

    fn owner(seed: u8) -> Did {
        Did::new(Network::Production, Name::of(&[seed]))
    }

    fn with(how_many: u8, sealing: u64, governance: u64) -> Entity {
        let mut body = alone([1; 32], Epoch::GENESIS);
        body.owners = (0..how_many).map(owner).collect();
        body.thresholds = Thresholds {
            routine: 1,
            sealing,
            governance,
        };
        body
    }

    #[test]
    fn a_network_that_has_just_opened_says_so_rather_than_listing_faults() {
        // The starting state of `SPECS.md §7.9` — one self-signed key — is one thing to say and not
        // three, and it is the one thing worth saying.
        let opened = alone([1; 32], Epoch::GENESIS);
        assert!(one_pair_of_hands(&opened));
        assert_eq!(counted(&opened), vec![Wanting::NobodyIsAnOwnerYet]);
        assert_eq!(
            fit(&opened, &BTreeMap::new()),
            vec![Wanting::NobodyIsAnOwnerYet]
        );
    }

    #[test]
    fn five_owners_with_three_sealing_and_three_governing_meets_what_the_record_can_see() {
        let body = with(5, 3, 3);
        assert!(counted(&body).is_empty());
        assert!(!one_pair_of_hands(&body));
    }

    #[test]
    fn four_owners_is_a_composition_that_does_not_survive_losing_one() {
        assert_eq!(
            counted(&with(4, 3, 3)),
            vec![Wanting::TooFewOwners { has: 4 }]
        );
    }

    #[test]
    fn sealing_below_three_is_named_because_the_seal_is_a_permission() {
        // Since `SPECS.md §9.4` the seal lets an organisation publish the shapes everybody uses, so
        // a low sealing threshold is how a hostile party is handed that.
        assert_eq!(
            counted(&with(5, 2, 3)),
            vec![Wanting::SealingTooLow { is: 2 }]
        );
    }

    #[test]
    fn governance_at_exactly_half_is_not_a_majority() {
        // Six owners and three governing means one half could change who the owners are, and the
        // other half would find out afterwards.
        assert_eq!(
            counted(&with(6, 3, 3)),
            vec![Wanting::GovernanceIsNotAMajority { is: 3, of: 6 }]
        );
        assert!(counted(&with(6, 3, 4)).is_empty());
    }

    #[test]
    fn half_the_owners_in_one_organisation_is_half_the_owners_in_one_place() {
        // Five owners of whom three share an employer are, in practice, three votes and one party.
        let owners: BTreeSet<Did> = (0..5).map(owner).collect();
        let mut where_they_are = BTreeMap::new();
        for seed in 0..3 {
            where_they_are.insert(
                owner(seed),
                Declared {
                    organisation: "One Company".to_owned(),
                    jurisdiction: format!("country {seed}"),
                },
            );
        }
        for seed in 3..5 {
            where_they_are.insert(
                owner(seed),
                Declared {
                    organisation: format!("company {seed}"),
                    jurisdiction: format!("country {seed}"),
                },
            );
        }
        assert_eq!(
            spread(&owners, &where_they_are),
            vec![Wanting::TooManyInOne {
                called: "One Company".to_owned(),
                has: 3,
                of: 5,
            }]
        );
    }

    #[test]
    fn one_jurisdiction_holding_half_of_them_is_named_too() {
        // A different failure from the one above and worth telling apart: one company is a
        // commercial dependency, one country is a legal one.
        let owners: BTreeSet<Did> = (0..4).map(owner).collect();
        let where_they_are = (0..4)
            .map(|seed| {
                (
                    owner(seed),
                    Declared {
                        organisation: format!("company {seed}"),
                        jurisdiction: if seed < 2 {
                            "one country".to_owned()
                        } else {
                            format!("country {seed}")
                        },
                    },
                )
            })
            .collect();
        assert_eq!(
            spread(&owners, &where_they_are),
            vec![Wanting::TooManyInOne {
                called: "one country".to_owned(),
                has: 2,
                of: 4,
            }]
        );
    }

    #[test]
    fn an_owner_nobody_declared_anything_about_counts_as_their_own_place() {
        // The generous reading, and deliberately so: counting silence as *somewhere already
        // counted* would let a composition pass by declaring less about itself.
        let owners: BTreeSet<Did> = (0..5).map(owner).collect();
        assert!(spread(&owners, &BTreeMap::new()).is_empty());
    }

    #[test]
    fn the_whole_of_it_is_the_record_and_the_declaration_together() {
        let body = with(5, 3, 3);
        let where_they_are = (0..5)
            .map(|seed| {
                (
                    owner(seed),
                    Declared {
                        organisation: "One Company".to_owned(),
                        jurisdiction: format!("country {seed}"),
                    },
                )
            })
            .collect();
        assert!(counted(&body).is_empty(), "the record alone is satisfied");
        assert_eq!(
            fit(&body, &where_they_are),
            vec![Wanting::TooManyInOne {
                called: "One Company".to_owned(),
                has: 5,
                of: 5,
            }],
            "and the declaration is what says it is one party wearing five names"
        );
    }
}
