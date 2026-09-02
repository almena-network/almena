//! The relationships this partner has, and the key of this end of each.
//!
//! **A relationship is not in the record.** It is not published, it has no chain, and nobody but
//! its two ends knows it exists — so it lives in the partner's directory and nowhere else. What is
//! kept is this end's identifier and key, and the far end's identifier, which already carries the
//! far end's keys and mediators inside it.
//!
//! The key is kept in the clear beside the identifier, which `crate::directory` says out loud: a
//! partner is a program on an organisation's machine, and the directory's permissions are what
//! protect it.

use std::collections::BTreeMap;

use crate::directory::{hex, unhex};
use crate::failed::Failed;
use crate::post::peer::Peer;

/// One relationship, from this end.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Relation {
    /// What this end is called in this relationship.
    pub mine: String,
    /// What the far end is called, or nothing where nobody has answered yet.
    pub theirs: Option<String>,
    /// The secret of this end's key, as hexadecimal.
    pub secret: String,
}

impl Relation {
    /// This end's key, ready to open and seal.
    ///
    /// # Errors
    ///
    /// `relations_key_invalid` for a secret that is not one, which nothing this program wrote is.
    pub fn key(&self) -> Result<p256::SecretKey, Failed> {
        let bytes = unhex(&self.secret).ok_or_else(|| Failed::new("relations_key_invalid"))?;
        p256::SecretKey::from_slice(&bytes).map_err(|_| Failed::new("relations_key_invalid"))
    }

    /// The far end, read out of its identifier.
    ///
    /// # Errors
    ///
    /// `relations_nobody_yet` where nobody has answered, `relations_far_end_unreadable` where what
    /// was kept is not a peer identifier.
    pub fn far_end(&self) -> Result<Peer, Failed> {
        let named = self
            .theirs
            .as_deref()
            .ok_or_else(|| Failed::new("relations_nobody_yet"))?;
        Peer::read(named).map_err(|_| Failed::new("relations_far_end_unreadable"))
    }
}

/// Every relationship this partner has.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Relations {
    /// By what this end is called in each, which is what a mediator routes on.
    by_mine: BTreeMap<String, Relation>,
}

impl Relations {
    /// A relationship minted here on a fresh key, with the far end named or not yet.
    #[must_use]
    pub fn minted(secret: &[u8; 32], mine: &Peer, theirs: Option<String>) -> Relation {
        Relation {
            mine: mine.to_did(),
            theirs,
            secret: hex(secret),
        }
    }

    /// Take up a relationship, or replace what was known about one.
    pub fn keep(&mut self, relation: Relation) {
        self.by_mine.insert(relation.mine.clone(), relation);
    }

    /// Every name this end answers to, which is what is declared to a mediator.
    #[must_use]
    pub fn addresses(&self) -> Vec<String> {
        self.by_mine.keys().cloned().collect()
    }

    /// Every relationship, in a fixed order.
    #[must_use]
    pub fn all(&self) -> Vec<&Relation> {
        self.by_mine.values().collect()
    }

    /// The relationship whose far end is that identifier.
    #[must_use]
    pub fn whose_far_end_is(&self, theirs: &str) -> Option<&Relation> {
        self.by_mine
            .values()
            .find(|relation| relation.theirs.as_deref() == Some(theirs))
    }

    /// The relationship a message addressed to `mine` belongs to.
    #[must_use]
    pub fn addressed(&self, mine: &str) -> Option<&Relation> {
        self.by_mine.get(mine)
    }

    /// The relationship a message addressed to `mine` and sealed by `sealed_by` belongs to.
    ///
    /// **Asked of every message that opens**, because opening one proves who sealed it and not who
    /// this relationship is with.
    ///
    /// # Errors
    ///
    /// `relations_not_one_of_mine`, `relations_nobody_yet`, `relations_not_from_them`.
    pub fn from_them(&self, mine: &str, sealed_by: &[u8]) -> Result<&Relation, Failed> {
        let relation = self
            .addressed(mine)
            .ok_or_else(|| Failed::new("relations_not_one_of_mine"))?;
        let theirs = relation.far_end()?;
        if theirs.seals.iter().any(|key| key == sealed_by) {
            Ok(relation)
        } else {
            Err(Failed::new("relations_not_from_them"))
        }
    }
}

/// The far end a first message on an introduction names, when it has proved it.
///
/// The identifier the message says it came from carries its own sealing keys, and the key that
/// actually sealed the message has to be one of them. Somebody who copied a code knows the address
/// and not the key.
#[must_use]
pub fn answered_by(claimed: &str, sealed_by: &[u8]) -> Option<String> {
    let far = Peer::read(claimed).ok()?;
    far.seals
        .iter()
        .any(|key| key == sealed_by)
        .then(|| far.to_did())
}

#[cfg(test)]
mod tests {
    use super::{Relations, answered_by};
    use crate::post::peer::{Peer, written};

    fn key(seed: u8) -> p256::SecretKey {
        p256::SecretKey::from_slice(&[seed.max(1); 32]).expect("a key")
    }

    fn end(seed: u8) -> Peer {
        Peer::on(
            &key(seed).public_key(),
            vec![("a:1".to_owned(), "12D3KooWnode".to_owned())],
        )
    }

    #[test]
    fn a_message_that_opens_and_came_from_somewhere_else_belongs_nowhere() {
        let mut relations = Relations::new_for_test();
        let relation = Relations::minted(&[1; 32], &end(1), Some(end(2).to_did()));
        relations.keep(relation.clone());
        assert!(
            relations
                .from_them(&relation.mine, &written(&key(2).public_key()))
                .is_ok()
        );
        assert_eq!(
            relations
                .from_them(&relation.mine, &written(&key(9).public_key()))
                .unwrap_err()
                .to_string(),
            "relations_not_from_them"
        );
        assert_eq!(
            relations
                .from_them("did:peer:2.Vznothing", &[])
                .unwrap_err()
                .to_string(),
            "relations_not_one_of_mine"
        );
        assert_eq!(relations.addresses(), vec![relation.mine.clone()]);
        assert_eq!(
            relations.whose_far_end_is(&end(2).to_did()),
            Some(&relation)
        );
        assert_eq!(relation.key().expect("a key"), key(1));
    }

    #[test]
    fn whoever_answers_an_introduction_has_to_prove_the_name_they_give() {
        let far = end(3);
        assert_eq!(
            answered_by(&far.to_did(), &written(&key(3).public_key())),
            Some(far.to_did())
        );
        assert_eq!(
            answered_by(&far.to_did(), &written(&key(9).public_key())),
            None
        );
        assert_eq!(answered_by("not an identifier", &[]), None);
    }

    impl Relations {
        fn new_for_test() -> Self {
            Self::default()
        }
    }
}
