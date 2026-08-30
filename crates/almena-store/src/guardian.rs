//! Who may freeze an account, and who may hand it back — without the record ever saying who.
//!
//! # The list is private, and what the record carries is a commitment
//!
//! `SPECS.md §11.4`. Guardians receive no seed, no credentials, and cannot act in somebody's name:
//! all they are is registered as able to authorise a recovery. **Registered without being named**,
//! because a public list of the people who can freeze somebody is a list of the people to go after.
//!
//! So the account's chain carries a Merkle root over one leaf per guardian, and a guardian who acts
//! shows their own leaf and the path to that root. They reveal themselves, which they were doing by
//! signing anyway; they reveal nothing about the others, because a sibling is a hash.
//!
//! **The same tree the node keeps its record in** (`crate::tree`), and reusing it is the point: two
//! Merkle implementations would agree for about a month, and the day they stopped, one node would
//! refuse a guardian every other node accepted.
//!
//! # A salt per guardian, and it is not decoration
//!
//! Without one, a leaf is the hash of an identifier — and identifiers are public and enumerable
//! (`SPECS.md §3`). Anybody could take the census, hash every account, and read the list straight
//! off the commitment. The salt is what makes the leaf unguessable, and it is per guardian so that
//! one revealing their own tells nobody anything about the rest.
//!
//! # What is public, said out loud
//!
//! **How many guardians there are, and how many it takes.** The threshold has to be public or
//! nobody can check that it was met, and the count is derivable from the length of any path anyway.
//! What stays private is *who*.
//!
//! # Guardians freeze; only the holder recovers
//!
//! `SPECS.md §11.4`, and it is `SPECS.md §1.8` said again: **what takes trust away is accepted
//! quickly; what grants it waits.** Freezing denies and grants nothing, so a quorum of guardians
//! does it and it lands at once. Recovering hands the account to somebody, so only the holder
//! starts it — if a guardian could, two of them colluding would rotate the identity to themselves —
//! and it waits where any device still in the holder's hands can refuse it.

use std::collections::BTreeSet;

use almena_format::cbor::Value;
use almena_format::identifier::Did;
use almena_format::operation::Operation;
use almena_suite::digest::{Digest, WIDTH};

use crate::chain::Refused;
use crate::tree::{Path, Tree, included};

/// How wide a salt is, in bytes.
///
/// **Sixteen.** It is what stops a leaf over a public, enumerable identifier being found by hashing
/// the census — which is the whole of what the commitment is protecting.
pub const SALT_WIDTH: usize = 16;

/// The most guardians one account may name.
///
/// **Bounded because a path is checked act by act**, and an account naming a million would be one
/// whose freeze costs every node a walk nobody asked for. Well past what anybody uses: the shape of
/// this is a handful of people who would recognise your voice.
pub const AT_MOST: u64 = 64;

/// Where each part of a `set_guardians` or a `recover` act sits.
///
/// **A range of their own, above the fields a holder act already uses**, so that the key an act is
/// about and the commitment it declares can never be read as each other. All odd: a reader that
/// skipped any of them could not claim to have applied the act — one that missed the commitment
/// would read `set_guardians` as an act that set nothing.
pub mod field {
    /// The root of the tree over the guardians.
    pub const COMMITMENT: u64 = 11;
    /// How many there are, which is the size the tree is checked against.
    pub const HOW_MANY: u64 = 13;
    /// How many of them it takes.
    pub const ENOUGH: u64 = 15;
    /// The proofs, one per guardian acting.
    pub const PROOFS: u64 = 17;
    /// The device key a recovery brings, which is the one the account is left operating with.
    pub const DEVICE: u64 = 19;
}

/// Where each part of one guardian's proof sits, inside the map that carries it.
pub mod proving {
    /// Whose proof it is.
    pub const GUARDIAN: u64 = 1;
    /// The salt their leaf was made with.
    pub const SALT: u64 = 3;
    /// Where they sit in the tree.
    pub const AT: u64 = 5;
    /// The hashes that carry their leaf up to the root.
    pub const PATH: u64 = 7;
}

/// What the record says about an account's guardians.
///
/// **A commitment and two numbers.** Everything else about them — who they are, how to reach them,
/// whether they still answer the phone — lives on the holder's own device (`SPECS.md §11.10`), and
/// the record is deliberately no help to anybody trying to find out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Guardians {
    /// The root of the tree over them.
    pub commitment: Digest,
    /// How many there are.
    pub how_many: u64,
    /// How many of them it takes to freeze, or to authorise a recovery.
    pub enough: u64,
}

/// One guardian's claim to be one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Proof {
    /// Whose it is.
    pub guardian: Did,
    /// The salt their leaf was made with.
    pub salt: Vec<u8>,
    /// Where they sit in the tree.
    pub at: u64,
    /// The hashes that carry their leaf up to the root.
    pub path: Path,
}

/// The leaf one guardian is committed to as.
///
/// **The salt in front of the identifier**, so that the leaf cannot be found by hashing a public
/// census — which is what an identifier is (`SPECS.md §3`).
#[must_use]
pub fn leaf(guardian: &Did, salt: &[u8]) -> Vec<u8> {
    let mut held = Vec::with_capacity(salt.len() + guardian.to_string().len());
    held.extend_from_slice(salt);
    held.extend_from_slice(guardian.to_string().as_bytes());
    held
}

/// The commitment over a list of guardians, in the order the holder wrote them.
///
/// **The holder's own order, kept**, because a proof names a position and a list somebody sorted on
/// the way in would put every guardian somewhere other than where their proof says.
#[must_use]
pub fn commit(guardians: &[(Did, Vec<u8>)]) -> Digest {
    let mut tree = Tree::new();
    for (guardian, salt) in guardians {
        tree.append(&leaf(guardian, salt));
    }
    tree.root()
}

impl Guardians {
    /// Whether that proof is one of this commitment's leaves.
    #[must_use]
    pub fn holds(&self, proof: &Proof) -> bool {
        let Ok(at) = usize::try_from(proof.at) else {
            return false;
        };
        let Ok(size) = usize::try_from(self.how_many) else {
            return false;
        };
        proof.salt.len() == SALT_WIDTH
            && included(
                &leaf(&proof.guardian, &proof.salt),
                at,
                size,
                &proof.path,
                &self.commitment,
            )
    }

    /// How many distinct guardians proved themselves in that act.
    ///
    /// **Distinct, and by identity rather than by position.** One guardian presenting two proofs of
    /// themselves — two positions, or the same one twice — is one guardian, and counting the proofs
    /// would let a single person reach a quorum on their own.
    #[must_use]
    pub fn counted(&self, proofs: &[Proof], signed: &BTreeSet<Did>) -> u64 {
        let mut seen: BTreeSet<&Did> = BTreeSet::new();
        for proof in proofs {
            // **And they have to have signed.** A proof is a claim to be a guardian; what makes it
            // an act of theirs is the signature, checked against a key their own chain authorises.
            if signed.contains(&proof.guardian) && self.holds(proof) {
                seen.insert(&proof.guardian);
            }
        }
        seen.len() as u64
    }

    /// Whether that is a quorum.
    #[must_use]
    pub fn enough_of_them(&self, counted: u64) -> bool {
        counted >= self.enough && self.enough > 0
    }
}

/// The guardians a `set_guardians` act declares.
///
/// # Errors
///
/// [`Refused::Malformed`] for a commitment that is not a digest, a count past what an account may
/// name, or a threshold that cannot be met — nought, or more than there are.
pub fn declared(operation: &Operation) -> Result<Guardians, Refused> {
    let Some(Value::Bytes(commitment)) = operation.payload.get(&field::COMMITMENT) else {
        return Err(Refused::Malformed);
    };
    let commitment: [u8; WIDTH] = commitment
        .as_slice()
        .try_into()
        .map_err(|_| Refused::Malformed)?;
    let (Some(Value::Uint(how_many)), Some(Value::Uint(enough))) = (
        operation.payload.get(&field::HOW_MANY),
        operation.payload.get(&field::ENOUGH),
    ) else {
        return Err(Refused::Malformed);
    };
    // **A threshold nobody can meet is not a configuration, it is a mistake nobody would notice.**
    // Nought would let one stranger with any proof at all freeze an account; more than there are
    // would leave guardians who can never do the one thing they are for.
    if *enough == 0 || *enough > *how_many || *how_many == 0 || *how_many > AT_MOST {
        return Err(Refused::Malformed);
    }
    Ok(Guardians {
        commitment: Digest::from_bytes(commitment),
        how_many: *how_many,
        enough: *enough,
    })
}

/// The proofs an act carries.
///
/// **Nothing rather than a refusal for an act with none**: an act with no proofs is one nobody is
/// claiming to be a guardian on, which is an ordinary act and not a broken one.
#[must_use]
pub fn proofs(operation: &Operation) -> Vec<Proof> {
    let Some(Value::Array(listed)) = operation.payload.get(&field::PROOFS) else {
        return Vec::new();
    };
    listed.iter().filter_map(one).collect()
}

/// One proof, out of what an act carries.
fn one(value: &Value) -> Option<Proof> {
    let Value::Map(held) = value else {
        return None;
    };
    let (
        Some(Value::Text(guardian)),
        Some(Value::Bytes(salt)),
        Some(Value::Uint(at)),
        Some(Value::Array(path)),
    ) = (
        held.get(&proving::GUARDIAN),
        held.get(&proving::SALT),
        held.get(&proving::AT),
        held.get(&proving::PATH),
    )
    else {
        return None;
    };
    let hashes = path
        .iter()
        .map(|step| match step {
            Value::Bytes(hash) => <[u8; WIDTH]>::try_from(hash.as_slice())
                .ok()
                .map(Digest::from_bytes),
            _ => None,
        })
        .collect::<Option<Vec<Digest>>>()?;
    Some(Proof {
        guardian: Did::parse(guardian).ok()?,
        salt: salt.clone(),
        at: *at,
        path: Path::of(hashes),
    })
}

/// How a proof is written into an act.
#[must_use]
pub fn carried(proof: &Proof) -> Value {
    Value::Map(
        [
            (proving::GUARDIAN, Value::Text(proof.guardian.to_string())),
            (proving::SALT, Value::Bytes(proof.salt.clone())),
            (proving::AT, Value::Uint(proof.at)),
            (
                proving::PATH,
                Value::Array(
                    proof
                        .path
                        .hashes()
                        .iter()
                        .map(|hash| Value::Bytes(hash.bytes().to_vec()))
                        .collect(),
                ),
            ),
        ]
        .into_iter()
        .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::{AT_MOST, Guardians, Proof, SALT_WIDTH, carried, commit, declared, leaf, proofs};
    use crate::chain::Refused;
    use crate::kind::Kind;
    use crate::tree::Tree;
    use almena_format::cbor::Value;
    use almena_format::identifier::{Did, Name, Network};
    use almena_format::operation::create;
    use almena_suite::digest::Digest;
    use almena_time::Epoch;
    use std::collections::{BTreeMap, BTreeSet};

    fn guardian(seed: u8) -> Did {
        Did::new(Network::Development, Name::of(&[seed; 8]))
    }

    fn salt(seed: u8) -> Vec<u8> {
        vec![seed; SALT_WIDTH]
    }

    /// A list of guardians, each with a salt of their own.
    fn listed(how_many: u8) -> Vec<(Did, Vec<u8>)> {
        (1..=how_many)
            .map(|seed| (guardian(seed), salt(seed + 100)))
            .collect()
    }

    /// The proof one of them holds.
    fn proof(held: &[(Did, Vec<u8>)], at: usize) -> Proof {
        let mut tree = Tree::new();
        for (who, salt) in held {
            tree.append(&leaf(who, salt));
        }
        Proof {
            guardian: held[at].0.clone(),
            salt: held[at].1.clone(),
            at: at as u64,
            path: tree.inclusion(at).expect("a path"),
        }
    }

    fn guardians(held: &[(Did, Vec<u8>)], enough: u64) -> Guardians {
        Guardians {
            commitment: commit(held),
            how_many: held.len() as u64,
            enough,
        }
    }

    #[test]
    fn a_guardian_proves_themselves_and_says_nothing_about_the_others() {
        // **What the commitment is for.** A public list of the people who can freeze somebody is a
        // list of the people to go after; a guardian who acts reveals themselves, which signing did
        // anyway, and their siblings stay hashes.
        let held = listed(5);
        let one = proof(&held, 2);
        let set = guardians(&held, 3);
        assert!(set.holds(&one));

        // Somebody who is not on the list, with a proof made up out of the same shape.
        let stranger = Proof {
            guardian: guardian(99),
            ..one.clone()
        };
        assert!(!set.holds(&stranger));
    }

    #[test]
    fn a_leaf_cannot_be_found_by_hashing_the_census() {
        // Identifiers are public and enumerable, so without a salt anybody could hash every account
        // and read the list straight off the commitment.
        let held = listed(3);
        let salted = commit(&held);
        let unsalted: Vec<(Did, Vec<u8>)> = held
            .iter()
            .map(|(who, _)| (who.clone(), Vec::new()))
            .collect();
        assert_ne!(salted, commit(&unsalted));

        // And one guardian's salt is theirs: revealing it does not open anybody else's leaf.
        let one = proof(&held, 0);
        let other = proof(&held, 1);
        assert_ne!(one.salt, other.salt);
    }

    #[test]
    fn one_guardian_presenting_two_proofs_is_one_guardian() {
        // Counting proofs rather than people would let a single person reach a quorum alone.
        let held = listed(4);
        let set = guardians(&held, 2);
        let one = proof(&held, 1);
        let signed = BTreeSet::from([one.guardian.clone()]);

        assert_eq!(set.counted(&[one.clone(), one.clone()], &signed), 1);
        assert!(!set.enough_of_them(1));
    }

    #[test]
    fn a_proof_from_somebody_who_did_not_sign_counts_as_nobody() {
        // A proof is a claim to be a guardian; what makes it an act of theirs is the signature.
        let held = listed(4);
        let set = guardians(&held, 2);
        let one = proof(&held, 0);
        let other = proof(&held, 3);

        assert_eq!(
            set.counted(&[one.clone(), other.clone()], &BTreeSet::new()),
            0
        );
        let both = BTreeSet::from([one.guardian.clone(), other.guardian.clone()]);
        assert_eq!(set.counted(&[one, other], &both), 2);
        assert!(set.enough_of_them(2));
    }

    #[test]
    fn a_threshold_nobody_can_meet_is_refused_rather_than_stored() {
        // Nought would let one stranger with any proof at all freeze an account; more than there
        // are would leave guardians who can never do the one thing they are for.
        let held = listed(3);
        let commitment = commit(&held);
        let setting = |how_many: u64, enough: u64| {
            create(
                Network::Development,
                Kind::HOLDER_SET_GUARDIANS.number(),
                1,
                Epoch::new(100),
                BTreeMap::from([
                    (
                        super::field::COMMITMENT,
                        Value::Bytes(commitment.bytes().to_vec()),
                    ),
                    (super::field::HOW_MANY, Value::Uint(how_many)),
                    (super::field::ENOUGH, Value::Uint(enough)),
                ]),
            )
        };
        assert_eq!(declared(&setting(3, 0)), Err(Refused::Malformed));
        assert_eq!(declared(&setting(3, 4)), Err(Refused::Malformed));
        assert_eq!(declared(&setting(0, 0)), Err(Refused::Malformed));
        assert_eq!(declared(&setting(AT_MOST + 1, 1)), Err(Refused::Malformed));

        let read = declared(&setting(3, 2)).expect("a set of guardians");
        assert_eq!(read.commitment, commitment);
        assert_eq!(read.enough, 2);
    }

    #[test]
    fn a_proof_reads_back_as_itself_and_rubbish_reads_as_nothing() {
        let held = listed(4);
        let one = proof(&held, 2);
        let act = create(
            Network::Development,
            Kind::HOLDER_FREEZE.number(),
            1,
            Epoch::new(100),
            BTreeMap::from([(
                super::field::PROOFS,
                Value::Array(vec![carried(&one), Value::Uint(9)]),
            )]),
        );
        let read = proofs(&act);
        assert_eq!(read.len(), 1, "and what is not a proof is not read as one");
        assert_eq!(read[0], one);

        let none = create(
            Network::Development,
            Kind::HOLDER_FREEZE.number(),
            1,
            Epoch::new(100),
            BTreeMap::new(),
        );
        assert!(proofs(&none).is_empty(), "an ordinary act carries none");
    }

    #[test]
    fn a_salt_of_another_width_is_not_a_salt() {
        // The width is what the unguessability is measured in, and a narrow one would be a leaf
        // somebody could find by trying.
        let held = listed(3);
        let set = guardians(&held, 2);
        let mut short = proof(&held, 0);
        short.salt.truncate(4);
        assert!(!set.holds(&short));
    }

    #[test]
    fn a_commitment_over_nobody_matches_nothing() {
        let set = Guardians {
            commitment: Digest::of(b"over nobody"),
            how_many: 1,
            enough: 1,
        };
        let held = listed(1);
        assert!(!set.holds(&proof(&held, 0)));
    }
}
