//! The DIDComm envelope: what a mediator carries and cannot read.
//!
//! **The authenticated form, always.** A message on an established relationship is only meaningful
//! if the recipient knows which of their counterparties it came from, so the key that wraps the
//! message key is agreed twice: once against a key that exists for this message only, and once
//! against the sender's own key for the relationship. A recipient who arrives at the same value
//! has established that whoever composed it holds the sender's key, without a signature.
//!
//! # What is in the open
//!
//! The header travels unsealed: the one-time key, and the sender's key for this relationship —
//! which a mediator can see and which says nothing about who the counterparty is, since a
//! relationship's keys are used nowhere else and are unpublished.
//!
//! # One derived key, one message
//!
//! The wrapping key is derived over the tag of the sealed body, so two messages between the same
//! two keys never share one, and a derivation recovered from one message is worth nothing against
//! the next.

use almena_credential::base64url;
use p256::elliptic_curve::sec1::ToEncodedPoint as _;

use crate::post::keywrap;
use crate::post::peer;
use crate::post::sealing::{self, About, NotSealed, Sealed};

/// What this is, so that nothing else reads it as something it is not.
const KIND: &str = "application/didcomm-encrypted+json";

/// How the key that opens it is agreed.
const AGREED: &str = "ECDH-1PU+A256KW";

/// How the body itself is sealed.
const SEALED_WITH: &str = "A256CBC-HS512";

/// How long the key that seals the body is: two halves, one to sign with and one to seal with.
const CONTENT_KEY: usize = sealing::KEY_WIDTH * 2;

/// How long the initial value the body is sealed under is.
const IV_WIDTH: usize = 16;

/// The largest envelope this will read: the other end is not this program.
const LARGEST: usize = 4 * 1024 * 1024;

/// One message, sealed, in the JSON serialisation DIDComm defines.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Envelope {
    /// The header, sealed to nothing and authenticated by everything.
    pub protected: String,
    /// One entry per key this was sealed to — one per device of the recipient.
    pub recipients: Vec<For>,
    /// What the body was sealed under.
    pub iv: String,
    /// The body.
    pub ciphertext: String,
    /// What holds the body and the header together.
    pub tag: String,
}

/// One recipient's copy of the key that opens the body.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct For {
    /// Which key of theirs this copy is for.
    pub header: Whose,
    /// The message key, wrapped so that only that key's holder can take it out.
    pub encrypted_key: String,
}

/// Which key a copy is for.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Whose {
    /// The key, written the way a relationship writes keys.
    pub kid: String,
}

/// The header, as it travels.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct Header {
    typ: String,
    alg: String,
    enc: String,
    epk: Key,
    skid: String,
}

/// A public key, as JOSE writes one.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct Key {
    kty: String,
    crv: String,
    x: String,
    y: String,
}

/// Why an envelope did not open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotOpened {
    /// It is not one of these at all.
    NotAnEnvelope,
    /// It is, and it is not for any key this end holds.
    NotForThisDevice,
    /// It is, it is for this end, and it did not open.
    WouldNotOpen,
}

/// The randomness one sealing draws, so that a sealing can also be replayed exactly.
///
/// **Drawn from the operating system when a message is sent**, and given by hand only by a test
/// that needs the same bytes twice — which is how a vector the holder's app can open is made.
#[derive(Debug, Clone)]
pub struct Drawn {
    /// The key the body is sealed with: one per message, never derived from anything.
    pub content: [u8; CONTENT_KEY],
    /// The initial value the body is sealed under.
    pub iv: [u8; IV_WIDTH],
    /// The one-time key the agreement runs against.
    pub once: [u8; 32],
}

impl Drawn {
    /// Fresh randomness from the operating system.
    ///
    /// # Errors
    ///
    /// [`NotSealed`] when the machine will not produce any, on which nothing may be sealed.
    pub fn fresh() -> Result<Self, NotSealed> {
        let mut content = [0u8; CONTENT_KEY];
        let mut iv = [0u8; IV_WIDTH];
        let mut once = [0u8; 32];
        for _ in 0..8 {
            getrandom::fill(&mut content).map_err(|_| NotSealed)?;
            getrandom::fill(&mut iv).map_err(|_| NotSealed)?;
            getrandom::fill(&mut once).map_err(|_| NotSealed)?;
            if p256::SecretKey::from_slice(&once).is_ok() {
                return Ok(Self { content, iv, once });
            }
        }
        Err(NotSealed)
    }
}

/// Seal a message to every key of a relationship's far end, with fresh randomness.
///
/// `from` is this end's own key for that relationship, `to` are the far end's — one per device.
///
/// # Errors
///
/// [`NotSealed`] when there is nobody to seal to, a key is written some way other than the one
/// way this writes them, or the machine will not produce randomness.
pub fn seal(from: &p256::SecretKey, to: &[Vec<u8>], body: &[u8]) -> Result<Envelope, NotSealed> {
    seal_with(from, to, body, &Drawn::fresh()?)
}

/// Seal a message with the randomness given, which is what makes a sealing repeatable.
///
/// # Errors
///
/// As [`seal`].
pub fn seal_with(
    from: &p256::SecretKey,
    to: &[Vec<u8>],
    body: &[u8],
    drawn: &Drawn,
) -> Result<Envelope, NotSealed> {
    if to.is_empty() {
        return Err(NotSealed);
    }
    let recipients = to
        .iter()
        .map(|key| peer::canonical(key).map_err(|_| NotSealed))
        .collect::<Result<Vec<_>, NotSealed>>()?;
    let once = p256::SecretKey::from_slice(&drawn.once).map_err(|_| NotSealed)?;
    let header = serde_json::to_vec(&Header {
        typ: KIND.to_owned(),
        alg: AGREED.to_owned(),
        enc: SEALED_WITH.to_owned(),
        epk: jose(&once.public_key()),
        skid: named(&from.public_key()),
    })
    .map_err(|_| NotSealed)?;
    let over = base64url::encode(&header);
    // **The body is sealed first**, because the key that wraps its key is derived over the tag.
    let sealed = sealing::seal(&drawn.content, &drawn.iv, body, over.as_bytes())?;
    let recipients = recipients
        .iter()
        .map(|to| {
            let kek = agreed(&shared(&once, to), &shared(from, to), &sealed.tag);
            let wrapped = keywrap::wrap(&kek, &drawn.content).map_err(|_| NotSealed)?;
            Ok(For {
                header: Whose { kid: named(to) },
                encrypted_key: base64url::encode(&wrapped),
            })
        })
        .collect::<Result<Vec<_>, NotSealed>>()?;
    Ok(Envelope {
        protected: over,
        recipients,
        iv: base64url::encode(&drawn.iv),
        ciphertext: base64url::encode(&sealed.bytes),
        tag: base64url::encode(&sealed.tag),
    })
}

/// Open one, with this end's own key for that relationship.
///
/// Hands back the message and the key that sent it, compressed, so that the caller can check the
/// sender is who this relationship is with. **It is not checked here**: which keys count as the
/// far end is what the relationship knows.
///
/// # Errors
///
/// [`NotOpened`].
pub fn open(mine: &p256::SecretKey, envelope: &Envelope) -> Result<(Vec<u8>, Vec<u8>), NotOpened> {
    let (once, sender) = header_of(envelope)?;
    let ours = named(&mine.public_key());
    let wrapped = envelope
        .recipients
        .iter()
        .find(|one| one.header.kid == ours)
        .ok_or(NotOpened::NotForThisDevice)?;
    let sealed = Sealed {
        bytes: bytes(&envelope.ciphertext)?,
        tag: bytes(&envelope.tag)?,
    };
    let iv = bytes(&envelope.iv)?;
    let wrapped = bytes(&wrapped.encrypted_key)?;
    let kek = agreed(&shared(mine, &once), &shared(mine, &sender), &sealed.tag);
    let content = keywrap::unwrap(&kek, &wrapped).map_err(|_| NotOpened::WouldNotOpen)?;
    let body = sealing::open(&content, &iv, &sealed, envelope.protected.as_bytes())
        .map_err(|NotSealed| NotOpened::WouldNotOpen)?;
    Ok((body, peer::written(&sender)))
}

/// The one-time key and the sender's key, out of a header this build knows how to read.
fn header_of(envelope: &Envelope) -> Result<(p256::PublicKey, p256::PublicKey), NotOpened> {
    let header = bytes(&envelope.protected)?;
    let read: Header = serde_json::from_slice(&header).map_err(|_| NotOpened::NotAnEnvelope)?;
    if read.typ != KIND || read.alg != AGREED || read.enc != SEALED_WITH {
        // **Refused, not attempted.** Reading an envelope whose header names a way of sealing this
        // build does not know, as if it named the one it does, is how a message ends up opened by
        // the wrong algorithm and believed.
        return Err(NotOpened::NotAnEnvelope);
    }
    let once = point(&read.epk).ok_or(NotOpened::NotAnEnvelope)?;
    let sender = read
        .skid
        .strip_prefix('z')
        .and_then(|body| peer::read_key(body).ok())
        .ok_or(NotOpened::NotAnEnvelope)?;
    Ok((once, sender))
}

/// What two keys agree on, which neither of them can reach alone.
fn shared(mine: &p256::SecretKey, theirs: &p256::PublicKey) -> Vec<u8> {
    p256::ecdh::diffie_hellman(mine.to_nonzero_scalar(), theirs.as_affine())
        .raw_secret_bytes()
        .to_vec()
}

/// The key that wraps the message key, agreed between the two ends.
///
/// Two agreements and not one: the one-time key's, then the sender's own. Both ends reach it from
/// opposite sides, which is why this takes the two shared values rather than the keys.
fn agreed(once: &[u8], between: &[u8], tag: &[u8]) -> [u8; 32] {
    let mut secret = once.to_vec();
    secret.extend_from_slice(between);
    let derived = sealing::derived(
        &secret,
        &About {
            algorithm: AGREED,
            sender: &[],
            recipient: &[],
            tag,
        },
        sealing::KEY_WIDTH,
    );
    let mut out = [0u8; 32];
    out.copy_from_slice(&derived);
    out
}

/// A key named the way a relationship names keys, so that a header and a peer identifier agree.
fn named(key: &p256::PublicKey) -> String {
    peer::multibase(&peer::written(key))
}

/// A public key, as JOSE writes one.
fn jose(key: &p256::PublicKey) -> Key {
    let point = key.to_encoded_point(false);
    Key {
        kty: "EC".to_owned(),
        crv: "P-256".to_owned(),
        x: base64url::encode(point.x().map_or(&[][..], |x| x)),
        y: base64url::encode(point.y().map_or(&[][..], |y| y)),
    }
}

/// The same, read back — and refused if it is not a point on the curve this speaks.
fn point(key: &Key) -> Option<p256::PublicKey> {
    if key.kty != "EC" || key.crv != "P-256" {
        return None;
    }
    let mut bytes = vec![0x04];
    bytes.extend_from_slice(&base64url::decode(&key.x).ok()?);
    bytes.extend_from_slice(&base64url::decode(&key.y).ok()?);
    p256::PublicKey::from_sec1_bytes(&bytes).ok()
}

/// One field, decoded, bounded.
fn bytes(text: &str) -> Result<Vec<u8>, NotOpened> {
    if text.len() > LARGEST {
        return Err(NotOpened::NotAnEnvelope);
    }
    base64url::decode(text).map_err(|_| NotOpened::NotAnEnvelope)
}

#[cfg(test)]
mod tests {
    use super::{Drawn, Envelope, NotOpened, NotSealed, open, seal, seal_with};
    use crate::post::peer::written;

    fn a_key(seed: u8) -> p256::SecretKey {
        p256::SecretKey::from_slice(&[seed.max(1); 32]).expect("a key")
    }

    fn public(key: &p256::SecretKey) -> Vec<u8> {
        written(&key.public_key())
    }

    #[test]
    fn a_message_seals_to_every_device_and_each_of_them_opens_it() {
        let sender = a_key(1);
        let (phone, laptop) = (a_key(2), a_key(3));
        let envelope = seal(
            &sender,
            &[public(&phone), public(&laptop)],
            b"an offer nobody else may read",
        )
        .expect("somebody to seal to");
        assert_eq!(envelope.recipients.len(), 2);
        for device in [&phone, &laptop] {
            assert_eq!(
                open(device, &envelope),
                Ok((b"an offer nobody else may read".to_vec(), public(&sender)))
            );
        }
    }

    #[test]
    fn a_device_this_was_not_sealed_to_is_told_that_and_not_something_else() {
        let envelope = seal(&a_key(1), &[public(&a_key(2))], b"a message").expect("sealed");
        assert_eq!(open(&a_key(9), &envelope), Err(NotOpened::NotForThisDevice));
    }

    #[test]
    fn the_mediator_carrying_it_cannot_read_it_nor_learn_who_sent_it() {
        let envelope = seal(&a_key(1), &[public(&a_key(2))], b"an offer").expect("sealed");
        let whole = serde_json::to_vec(&envelope).expect("json");
        assert!(!whole.windows(8).any(|window| window == b"an offer"));
        assert!(
            !envelope
                .protected
                .contains(&super::named(&a_key(1).public_key())[1..]),
            "the sender's key is in the header only in the spelling the header uses"
        );
    }

    #[test]
    fn changing_anything_at_all_stops_it_opening() {
        let phone = a_key(2);
        let sealed = seal(&a_key(1), &[public(&phone)], b"a message").expect("sealed");
        let flipped = |text: &str| {
            let mut out: Vec<char> = text.chars().collect();
            out[0] = if out[0] == 'A' { 'B' } else { 'A' };
            out.into_iter().collect::<String>()
        };
        for spoiled in [
            Envelope {
                ciphertext: flipped(&sealed.ciphertext),
                ..sealed.clone()
            },
            Envelope {
                tag: flipped(&sealed.tag),
                ..sealed.clone()
            },
            Envelope {
                iv: flipped(&sealed.iv),
                ..sealed.clone()
            },
            Envelope {
                protected: flipped(&sealed.protected),
                ..sealed.clone()
            },
        ] {
            assert!(open(&phone, &spoiled).is_err());
        }
    }

    #[test]
    fn two_messages_between_the_same_two_keys_do_not_share_a_wrapping_key() {
        let (sender, phone) = (a_key(1), a_key(2));
        let one = seal(&sender, &[public(&phone)], b"the first").expect("sealed");
        let other = seal(&sender, &[public(&phone)], b"the second").expect("sealed");
        assert_ne!(
            one.recipients[0].encrypted_key,
            other.recipients[0].encrypted_key
        );
    }

    #[test]
    fn a_header_naming_a_way_of_sealing_this_build_does_not_know_is_refused() {
        let phone = a_key(2);
        let sealed = seal(&a_key(1), &[public(&phone)], b"a message").expect("sealed");
        let header = almena_credential::base64url::decode(&sealed.protected).expect("bytes");
        let header = String::from_utf8(header)
            .expect("text")
            .replace("A256CBC-HS512", "A128CBC-HS256");
        assert_eq!(
            open(
                &phone,
                &Envelope {
                    protected: almena_credential::base64url::encode(header.as_bytes()),
                    ..sealed
                }
            ),
            Err(NotOpened::NotAnEnvelope)
        );
    }

    #[test]
    fn there_is_nobody_to_seal_to() {
        assert_eq!(seal(&a_key(1), &[], b"a message"), Err(NotSealed));
        for not_a_key in [vec![0x02; 32], vec![0x05; 33], Vec::new()] {
            assert_eq!(seal(&a_key(1), &[not_a_key], b"a message"), Err(NotSealed));
        }
    }

    #[test]
    fn the_same_randomness_seals_the_same_bytes_twice() {
        // What makes a vector: a sealing that can be written down, and opened by the other side.
        let drawn = Drawn {
            content: [5; 64],
            iv: [6; 16],
            once: [7; 32],
        };
        let one = seal_with(&a_key(1), &[public(&a_key(2))], b"hello", &drawn).expect("sealed");
        let two = seal_with(&a_key(1), &[public(&a_key(2))], b"hello", &drawn).expect("sealed");
        assert_eq!(one, two);
        assert_eq!(open(&a_key(2), &one).expect("opens").0, b"hello");
    }

    #[test]
    fn both_ends_arrive_at_the_same_wrapping_key() {
        let (sender, recipient, once) = (a_key(1), a_key(2), a_key(3));
        assert_eq!(
            super::agreed(
                &super::shared(&once, &recipient.public_key()),
                &super::shared(&sender, &recipient.public_key()),
                b"a tag"
            ),
            super::agreed(
                &super::shared(&recipient, &once.public_key()),
                &super::shared(&recipient, &sender.public_key()),
                b"a tag"
            )
        );
    }
}
