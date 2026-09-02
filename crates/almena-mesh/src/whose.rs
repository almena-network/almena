//! Getting a node's key back out of the name it answers to on the mesh.
//!
//! **This is what makes a root received over the mesh worth anything.** A root is only believable
//! against the key of the node that signed it, and until now nothing could say which key that was:
//! a name in the record is the hash of an act, not a key, and asking the record would mean
//! resolving something before you can check the thing you would resolve it with.
//!
//! Here there is nothing to resolve. A mesh name **contains** the key rather than a digest of it —
//! keys of this size are carried whole — and the connection it arrived over already proved that
//! whoever is on the other end holds it. So a root from that peer can be held to that key without
//! asking anybody anything.
//!
//! # What this does not establish
//!
//! Only that a peer signed what it sent. Whether that peer is a node the network has heard of, and
//! whether the name inside its root is one it is entitled to use, are different questions with
//! different answers — and they are answered in the record, not here.

use almena_suite::ed25519;
use libp2p::PeerId;

/// The wrapper's way of saying no hash was used, which is the only kind a key can be read out of.
const NO_HASH: u64 = 0x00;

/// How long the message inside is: two field markers, a length, and the key.
const MESSAGE: usize = 4 + ed25519::PUBLIC_KEY_WIDTH;

/// The key a mesh name carries.
///
/// [`None`] for a name that carries a digest instead — which is what a name for any other kind of
/// key looks like, and which nothing here can check a signature against.
#[must_use]
pub fn key_of(peer: &PeerId) -> Option<[u8; ed25519::PUBLIC_KEY_WIDTH]> {
    let multihash = peer.as_ref();
    if multihash.code() != NO_HASH {
        return None;
    }

    let digest = multihash.digest();
    if digest.len() != MESSAGE {
        return None;
    }
    // Past the two field markers and the length, the rest is the key.
    digest[4..].try_into().ok()
}

/// The name a key answers to on the mesh.
///
/// **The other direction, for dialling somebody the record names.** The record says where a node
/// can be reached and what key it announced itself with; an address is only worth dialling with
/// that key on the end of it, or whoever took the host and port would be spoken to as that node.
/// [`None`] for bytes that are not a key, which the record would not hold.
#[must_use]
pub fn name_of(key: &[u8; ed25519::PUBLIC_KEY_WIDTH]) -> Option<PeerId> {
    libp2p::identity::ed25519::PublicKey::try_from_bytes(key)
        .ok()
        .map(|public| libp2p::identity::PublicKey::from(public).to_peer_id())
}

#[cfg(test)]
mod tests {
    use super::{key_of, name_of};
    use almena_suite::ed25519;

    fn key(seed: u8) -> ed25519::SigningKey {
        ed25519::SigningKey::from_secret([seed; 32])
    }

    #[test]
    fn the_key_comes_back_out_of_the_name_it_went_into() {
        // The whole point: a root that arrives from a peer can be held to that peer's key without
        // resolving anything, because the name already carries it.
        for seed in [0u8, 1, 9, 200, 255] {
            let key = key(seed);
            let peer = crate::identity(&key).expect("a key").public().to_peer_id();

            assert_eq!(
                key_of(&peer),
                Some(key.verifying_key().bytes()),
                "seed {seed}"
            );
        }
    }

    #[test]
    fn two_names_give_two_keys() {
        let one = crate::identity(&key(1))
            .expect("a key")
            .public()
            .to_peer_id();
        let other = crate::identity(&key(2))
            .expect("a key")
            .public()
            .to_peer_id();
        assert_ne!(key_of(&one), key_of(&other));
    }

    #[test]
    fn the_name_made_from_a_key_is_the_one_the_key_answers_to() {
        // What lets a node dial somebody the record names: the record holds the key, the socket
        // wants the name, and the two have to be the same name the node itself works out.
        for seed in [0u8, 1, 9, 200, 255] {
            let key = key(seed);
            let answers_to = crate::identity(&key).expect("a key").public().to_peer_id();
            assert_eq!(
                name_of(&key.verifying_key().bytes()),
                Some(answers_to),
                "seed {seed}"
            );
            assert_eq!(
                name_of(&key.verifying_key().bytes()).and_then(|peer| key_of(&peer)),
                Some(key.verifying_key().bytes()),
                "and the key comes back out of it"
            );
        }
    }

    #[test]
    fn a_name_that_carries_a_digest_carries_no_key() {
        // Names for other kinds of key are hashed rather than carried whole, and there is nothing
        // in one to check a signature against. Saying so beats guessing.
        assert_eq!(key_of(&libp2p::PeerId::random()), None);
    }
}
