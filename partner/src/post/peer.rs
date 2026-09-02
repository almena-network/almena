//! `did:peer`, which is what a relationship is called.
//!
//! **Numalgo 2**, the form that carries keys and services inline: each part opens with a letter
//! saying what follows — `V` for the key that signs, `E` for the key that seals, `S` for a service
//! — and the rest is a multibase key or the service as base64url text. A relationship is not an
//! object of the record: nothing has to be looked up to write to one, and a different key is a
//! different identifier, which is why adding a device is a rotation and not an edit.
//!
//! # One spelling of a key, or a relationship has two names
//!
//! A key is written compressed and read back only in that spelling. The library reads more forms
//! than this writes, and a key with two spellings would be two identifiers naming one relationship.

use almena_credential::base64url;
use almena_format::identifier::{base58, unbase58};
use p256::elliptic_curve::sec1::ToEncodedPoint as _;

/// The identifier every relationship is named by.
const METHOD: &str = "did:peer:2";

/// What each part of a peer identifier is for.
mod purpose {
    /// The key that signs.
    pub const SIGNS: char = 'V';
    /// The key that seals.
    pub const SEALS: char = 'E';
    /// Where to deliver.
    pub const SERVICE: char = 'S';
}

/// What a P-256 public key is called in the table of key types, so that a reader never decides
/// what a key is by measuring it.
const P256_PUBLIC: [u8; 2] = [0x80, 0x24];

/// One end of a relationship, as its counterparty sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Peer {
    /// The keys this end signs with, one per device, compressed.
    pub signs: Vec<Vec<u8>>,
    /// The keys this end can be sealed to, one per device, in the same order.
    pub seals: Vec<Vec<u8>>,
    /// Where messages for this end are delivered, in the order they should be tried: each
    /// mediator's address (`host:port`) and the identity of the node that runs it.
    ///
    /// **The node's identity beside each address**, because nobody signs a node's certificate: a
    /// sender dialling a mediator verifies the connection against that node's own key and against
    /// nothing else, so an address without the key is one a sender cannot safely write to. The two
    /// travel inside the identifier as one service, `address peer` with one space between, exactly
    /// as the holder's app writes it — and a service without the second half is not one this reads.
    pub delivered_to: Vec<(String, String)>,
}

/// Why something is not a peer identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotAPeer {
    /// It does not open with the method and the number.
    NotThisMethod,
    /// A part is not the shape its letter says it is.
    Unreadable,
    /// It has no key to sign with, none to seal to, or the two lists do not pair up.
    Incomplete,
}

impl Peer {
    /// One end carried on one key, used both to sign and to seal, delivered at those services.
    ///
    /// **What a relationship minted here looks like**: one key made for it and used in no other,
    /// which is what keeps two counterparties from putting two identifiers side by side.
    #[must_use]
    pub fn on(key: &p256::PublicKey, delivered_to: Vec<(String, String)>) -> Self {
        let named = written(key);
        Self {
            signs: vec![named.clone()],
            seals: vec![named],
            delivered_to,
        }
    }

    /// Write this end out as the identifier a counterparty holds.
    #[must_use]
    pub fn to_did(&self) -> String {
        let mut out = METHOD.to_owned();
        for (letter, keys) in [(purpose::SIGNS, &self.signs), (purpose::SEALS, &self.seals)] {
            for key in keys {
                out.push('.');
                out.push(letter);
                out.push_str(&multibase(key));
            }
        }
        for (address, peer) in &self.delivered_to {
            out.push('.');
            out.push(purpose::SERVICE);
            out.push_str(&base64url::encode(format!("{address} {peer}").as_bytes()));
        }
        out
    }

    /// Read one back.
    ///
    /// # Errors
    ///
    /// [`NotAPeer`], saying which of the three ways it is not one.
    pub fn read(did: &str) -> Result<Self, NotAPeer> {
        let rest = did
            .strip_prefix(METHOD)
            .ok_or(NotAPeer::NotThisMethod)?
            .strip_prefix('.')
            .ok_or(NotAPeer::NotThisMethod)?;
        let mut peer = Self {
            signs: Vec::new(),
            seals: Vec::new(),
            delivered_to: Vec::new(),
        };
        for part in rest.split('.') {
            let mut letters = part.chars();
            let letter = letters.next().ok_or(NotAPeer::Unreadable)?;
            let body = letters.as_str();
            match letter {
                purpose::SIGNS => peer.signs.push(written(&keyed(body)?)),
                purpose::SEALS => peer.seals.push(written(&keyed(body)?)),
                purpose::SERVICE => {
                    let service = base64url::decode(body)
                        .ok()
                        .and_then(|bytes| String::from_utf8(bytes).ok())
                        .ok_or(NotAPeer::Unreadable)?;
                    // An address and the node it is pinned to, or it is not somewhere a sender
                    // can write: without the key there is nothing to verify the mediator against.
                    let (address, node) = service
                        .split_once(' ')
                        .filter(|(address, node)| !address.is_empty() && !node.is_empty())
                        .ok_or(NotAPeer::Unreadable)?;
                    peer.delivered_to
                        .push((address.to_owned(), node.to_owned()));
                }
                // A letter this build has no meaning for is passed over, so that a counterparty
                // running a later build can still be reached by this one.
                _ => {}
            }
        }
        if peer.signs.is_empty() || peer.seals.is_empty() || peer.signs.len() != peer.seals.len() {
            return Err(NotAPeer::Incomplete);
        }
        Ok(peer)
    }
}

/// The key inside a `z`-prefixed multibase part.
fn keyed(body: &str) -> Result<p256::PublicKey, NotAPeer> {
    read_key(body.strip_prefix('z').ok_or(NotAPeer::Unreadable)?)
}

/// A public key with its type in front of it, written the way the method writes keys.
///
/// Shared with the envelope, so that a key named in a header and a key named in a relationship
/// are named the same.
#[must_use]
pub fn multibase(key: &[u8]) -> String {
    let mut bytes = P256_PUBLIC.to_vec();
    bytes.extend_from_slice(key);
    format!("z{}", base58(&bytes))
}

/// A public key, read from the one way this program writes them.
///
/// # Errors
///
/// [`NotAPeer::Unreadable`] for anything that is not a point, or is one written another way.
pub fn canonical(key: &[u8]) -> Result<p256::PublicKey, NotAPeer> {
    let public = p256::PublicKey::from_sec1_bytes(key).map_err(|_| NotAPeer::Unreadable)?;
    if written(&public) != key {
        return Err(NotAPeer::Unreadable);
    }
    Ok(public)
}

/// The key inside one part, checked against the type it says it is.
///
/// # Errors
///
/// [`NotAPeer::Unreadable`] for a type this build does not know — refused and never guessed at —
/// or a key that is not one.
pub fn read_key(body: &str) -> Result<p256::PublicKey, NotAPeer> {
    let bytes = unbase58(body).ok_or(NotAPeer::Unreadable)?;
    let (kind, key) = bytes.split_at_checked(2).ok_or(NotAPeer::Unreadable)?;
    if kind != P256_PUBLIC {
        return Err(NotAPeer::Unreadable);
    }
    canonical(key)
}

/// The public half of a key, as the bytes everything here writes keys in: compressed.
#[must_use]
pub fn written(key: &p256::PublicKey) -> Vec<u8> {
    key.to_encoded_point(true).as_bytes().to_vec()
}

#[cfg(test)]
mod tests {
    use super::{NotAPeer, Peer, written};

    fn a_key(seed: u8) -> Vec<u8> {
        let secret = p256::SecretKey::from_slice(&[seed.max(1); 32]).expect("a key");
        written(&secret.public_key())
    }

    /// A node's identity as the zone writes it.
    const NODE: &str = "12D3KooWSukJS2ezumqJjEaKRLPczQ1XenQ3AzqKXMXdKUmEkFe1";

    fn a_peer() -> Peer {
        Peer {
            signs: vec![a_key(1)],
            seals: vec![a_key(2)],
            delivered_to: vec![("madrid.dev.almena.network:8790".to_owned(), NODE.to_owned())],
        }
    }

    #[test]
    fn a_relationship_survives_being_written_down_and_read_back() {
        let peer = a_peer();
        let did = peer.to_did();
        assert!(did.starts_with("did:peer:2."), "{did}");
        assert_eq!(Peer::read(&did), Ok(peer));
    }

    #[test]
    fn the_spelling_agrees_with_the_holder_s_app_letter_for_letter() {
        // **The same string the holder's app writes for the same keys**, which is what makes an
        // identifier minted there readable here: the purpose letters, the multibase prefix, the
        // key type in front of the key, and base64url without padding over `address peer` — one
        // space — for the service.
        let did = a_peer().to_did();
        let parts: Vec<&str> = did.split('.').collect();
        assert_eq!(parts[0], "did:peer:2");
        assert!(parts[1].starts_with("Vz"));
        assert!(parts[2].starts_with("Ez"));
        assert_eq!(
            parts[3],
            "SbWFkcmlkLmRldi5hbG1lbmEubmV0d29yazo4NzkwIDEyRDNLb29XU3VrSlMyZXp1bXFKakVhS1JMUGN6UTFYZW5RM0F6cUtYTVhkS1VtRWtGZTE"
        );
    }

    #[test]
    fn a_service_without_the_node_s_identity_is_not_somewhere_to_write() {
        // An address alone is what the app used to write and now refuses: without the key of the
        // node behind it, a sender has nothing to verify the mediator against.
        let alone = super::base64url::encode(b"https://madrid.dev.almena.network");
        let mut bare = a_peer().to_did();
        let cut = bare.rfind(".S").expect("a service");
        bare.truncate(cut);
        assert_eq!(
            Peer::read(&format!("{bare}.S{alone}")),
            Err(NotAPeer::Unreadable)
        );
        let half = super::base64url::encode(b"madrid.dev.almena.network:8790 ");
        assert_eq!(
            Peer::read(&format!("{bare}.S{half}")),
            Err(NotAPeer::Unreadable)
        );
    }

    #[test]
    fn every_device_and_every_mediator_is_carried() {
        let peer = Peer {
            signs: vec![a_key(1), a_key(3)],
            seals: vec![a_key(2), a_key(4)],
            delivered_to: vec![
                ("a:1".to_owned(), NODE.to_owned()),
                ("b:2".to_owned(), NODE.to_owned()),
            ],
        };
        assert_eq!(Peer::read(&peer.to_did()), Ok(peer));
    }

    #[test]
    fn a_relationship_that_cannot_pair_its_keys_is_refused() {
        let lopsided = Peer {
            signs: vec![a_key(1), a_key(3)],
            seals: vec![a_key(2)],
            ..a_peer()
        };
        assert_eq!(Peer::read(&lopsided.to_did()), Err(NotAPeer::Incomplete));
    }

    #[test]
    fn a_part_this_build_has_no_meaning_for_is_passed_over() {
        let peer = a_peer();
        assert_eq!(
            Peer::read(&format!("{}.Iznothing", peer.to_did())),
            Ok(peer)
        );
    }

    #[test]
    fn a_key_of_another_type_or_another_spelling_does_not_get_in() {
        let mut wrong = vec![0xedu8, 0x01];
        wrong.extend_from_slice(&[7; 32]);
        let written = almena_format::identifier::base58(&wrong);
        assert_eq!(
            Peer::read(&format!("did:peer:2.Vz{written}.Ez{written}")),
            Err(NotAPeer::Unreadable)
        );
        for bad in [[0x05; 33].to_vec(), [0x02; 32].to_vec()] {
            let mut bytes = super::P256_PUBLIC.to_vec();
            bytes.extend_from_slice(&bad);
            let written = almena_format::identifier::base58(&bytes);
            assert_eq!(
                Peer::read(&format!("did:peer:2.Vz{written}.Ez{written}")),
                Err(NotAPeer::Unreadable)
            );
        }
        assert_eq!(
            Peer::read("did:almena:dev:zNobody"),
            Err(NotAPeer::NotThisMethod)
        );
    }
}
