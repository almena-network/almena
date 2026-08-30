//! The freshness rule: which version of a list may be used, and what to say when none can be.
//!
//! # Why latency stopped being a security promise
//!
//! `SPECS.md §10.2`. Two things travel at different speeds when an issuer revokes: the **hash** of
//! the new version, which goes into the log signed and is accepted on a single source, and the
//! **bytes**, which have to reach the replicas. The first is fast; the second is not.
//!
//! And that is enough for the worst case, because **a verifier holding the hash knows when the
//! bytes it was handed are old**. Accepting a revoked credential *believing the list is current*
//! cannot happen. What is left is availability, and availability is said as availability
//! (`SPECS.md §17.12`) rather than dressed up as a verdict about the credential.
//!
//! # Replica first, and the publication node only if the replica is stale
//!
//! Going to the source every time would tell the issuer **when and how often** its credentials are
//! verified — a signal it has none of today. It would not learn which credential, since the index
//! does not travel and a cohort is shared by many, but it would be information about its holders
//! that did not exist before.
//!
//! # What the hash does not cover
//!
//! Somebody hiding the revocation entirely. A node that withholds the new version's hash asserts
//! nothing false — it serves a log where that revocation simply is not there, and *no revocation*
//! and *it was kept from me* look identical. What there is against that is asking several nodes and
//! the fact that being behind is measured and published; it is said here rather than left implied.

use almena_credential::verify::Revocation;
use almena_suite::digest::Digest;

use crate::list::List;

/// What was reached when the verifier went looking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reached {
    /// The freshest version hash the record showed, where the record could be read at all.
    ///
    /// **[`None`] is *the record could not be read***, not *there is no list*: the second is a claim
    /// about the issuer and the first is about this verifier's reach.
    pub freshest: Option<Digest>,
    /// The bytes somebody served, where anybody served any.
    pub served: Option<List>,
}

/// What a verifier may conclude about a credential's revocation.
///
/// **Rule 1 of `SPECS.md §10.2`**, as a function: a version older than the freshest hash in sight
/// is never used, and where none can be had the answer is *not verified* rather than *not valid*.
#[must_use]
pub fn what_is_known(reached: &Reached, index: u64) -> Revocation {
    let (Some(freshest), Some(served)) = (reached.freshest, reached.served.as_ref()) else {
        return Revocation::Unavailable;
    };
    if served.version() != freshest {
        // It is not that the list is wrong: it is that it is not the one the record names, and
        // using it would be using a version this verifier can see has been superseded.
        return Revocation::Stale;
    }
    Revocation::Fresh {
        revoked: served.revoked(index),
    }
}

/// Where the bytes are asked for, and in what order.
///
/// **A replica first, and the issuer's own node only when the replica does not match**
/// (`SPECS.md §10.2`). The publication node is a hint and never the only source: if it were
/// required, every one of an issuer's revocations would hang on one point — and a hostile
/// publication node can refuse, it cannot lie, because the hash decides.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ask {
    /// A replica: any of them, because the hash decides who was right.
    Replica,
    /// The issuer's declared publication node, once a replica has come back stale.
    Publication,
    /// Nowhere else to ask.
    Nowhere,
}

/// What to do next, given what has been reached so far.
#[must_use]
pub fn next(reached: &Reached, asked: Option<Ask>) -> Ask {
    match asked {
        // Nothing asked yet: a replica, always.
        None => Ask::Replica,
        Some(Ask::Replica)
            if reached
                .served
                .as_ref()
                .zip(reached.freshest)
                .is_none_or(|(served, freshest)| served.version() != freshest) =>
        {
            Ask::Publication
        }
        Some(_) => Ask::Nowhere,
    }
}

#[cfg(test)]
mod tests {
    use super::{Ask, Reached, next, what_is_known};
    use crate::list::List;
    use almena_credential::verify::Revocation;
    use almena_suite::digest::Digest;

    #[test]
    fn a_list_older_than_the_hash_in_sight_is_never_used() {
        // **Rule 1.** Accepting a revoked credential believing the list is current cannot happen;
        // what is left is availability, and it is said as availability.
        let mut fresh = List::empty();
        fresh.revoke(7);
        let old = List::empty();

        assert_eq!(
            what_is_known(
                &Reached {
                    freshest: Some(fresh.version()),
                    served: Some(old),
                },
                7
            ),
            Revocation::Stale
        );
        assert_eq!(
            what_is_known(
                &Reached {
                    freshest: Some(fresh.version()),
                    served: Some(fresh.clone()),
                },
                7
            ),
            Revocation::Fresh { revoked: true }
        );
        assert_eq!(
            what_is_known(
                &Reached {
                    freshest: Some(fresh.version()),
                    served: Some(fresh),
                },
                8
            ),
            Revocation::Fresh { revoked: false }
        );
    }

    #[test]
    fn a_record_nobody_could_read_is_not_a_credential_nobody_revoked() {
        // Two different facts, and only one of them is about the issuer.
        assert_eq!(
            what_is_known(
                &Reached {
                    freshest: None,
                    served: Some(List::empty()),
                },
                7
            ),
            Revocation::Unavailable
        );
        assert_eq!(
            what_is_known(
                &Reached {
                    freshest: Some(Digest::of(b"a version")),
                    served: None,
                },
                7
            ),
            Revocation::Unavailable
        );
    }

    #[test]
    fn a_replica_is_asked_first_and_the_issuers_own_node_only_when_it_has_to_be() {
        // Going to the source every time would tell an issuer when and how often its credentials
        // are verified, which is a signal it has none of today.
        let fresh = List::empty();
        let matching = Reached {
            freshest: Some(fresh.version()),
            served: Some(fresh),
        };
        assert_eq!(next(&matching, None), Ask::Replica);
        assert_eq!(
            next(&matching, Some(Ask::Replica)),
            Ask::Nowhere,
            "a replica that matched is the end of it"
        );

        let mut newer = List::empty();
        newer.revoke(1);
        let stale = Reached {
            freshest: Some(newer.version()),
            served: Some(List::empty()),
        };
        assert_eq!(next(&stale, Some(Ask::Replica)), Ask::Publication);
        assert_eq!(next(&stale, Some(Ask::Publication)), Ask::Nowhere);
    }
}
