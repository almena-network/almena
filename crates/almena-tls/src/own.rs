//! The certificate a node makes from its own key, and how to read the key back out of one.
//!
//! A node has one key: it signs roots with it, it is named by it in the record, and it answers to
//! it on the mesh. Serving the interface under a *second* key would be a second census to keep in
//! step with the first, so the certificate a node presents is built around the same one — the
//! SubjectPublicKeyInfo **is** the node's Ed25519 public key, and the certificate is signed by that
//! key and by nobody else.
//!
//! # Why there is no authority in it
//!
//! Whoever dials a node already knows who they expect to answer: the zone published the node's
//! identity beside its address, and the record carries the same key in the node's own chain. A
//! certificate authority would add a third party vouching for a name, when the caller does not
//! care about the name — it cares that the key at the other end is the one it was told about.
//! So the client pins: it reads the key out of the certificate and compares. That is a smaller
//! claim than a chain of trust makes, and it is the one this platform can actually check.
//!
//! # Why the bytes are written here by hand
//!
//! What has to come out is one fixed shape — X.509 v3, one algorithm, one name, one validity, no
//! extensions — and it is a few hundred bytes. A library that generates certificates in general
//! would bring a DER encoder, a time library and a policy vocabulary into a crate whose whole job
//! is to produce this one thing, and the shape would then be decided by that library's defaults
//! rather than by something a reader can see. Every tag and every length is here.
//!
//! **A pure function of the key.** The same secret gives the same bytes every time, on any
//! machine: nothing is drawn at random and no clock is read. That is what makes it testable
//! against a fixed vector, and what lets a node restart without its certificate changing.
//!
//! # The validity is a formality, and it is written to say so
//!
//! The certificate says it is good from the start of 2025 until the end of the calendar. A pin
//! does not ask when a certificate expires — the node's key is the node's identity for as long as
//! the node exists — and an expiry date that was not enforced by anybody would be a date that
//! was quietly wrong one day. The far end of the calendar is the way X.509 itself spells *no
//! expiry*.

use almena_suite::ed25519::{self, PUBLIC_KEY_WIDTH};
use rustls_pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};

/// The tags of the DER shapes this certificate is made of.
mod tag {
    pub const INTEGER: u8 = 0x02;
    pub const BIT_STRING: u8 = 0x03;
    pub const OCTET_STRING: u8 = 0x04;
    pub const OBJECT_IDENTIFIER: u8 = 0x06;
    pub const UTF8_STRING: u8 = 0x0C;
    pub const UTC_TIME: u8 = 0x17;
    pub const GENERALIZED_TIME: u8 = 0x18;
    pub const SEQUENCE: u8 = 0x30;
    pub const SET: u8 = 0x31;
    /// `[0] EXPLICIT`, which is where a certificate's version goes.
    pub const VERSION: u8 = 0xA0;
}

/// The object identifier of Ed25519 (`1.3.101.112`), as an encoded element.
///
/// The one algorithm, both as what the certificate is signed with and as what the key inside it
/// is. Its `AlgorithmIdentifier` carries **no parameters** — not even an explicit absence — which
/// is why a reader can compare the whole identifier byte for byte.
const ID_ED25519: [u8; 5] = [tag::OBJECT_IDENTIFIER, 0x03, 0x2B, 0x65, 0x70];

/// The object identifier of `commonName` (`2.5.4.3`), as an encoded element.
const COMMON_NAME: [u8; 5] = [tag::OBJECT_IDENTIFIER, 0x03, 0x55, 0x04, 0x03];

/// What the certificate calls its subject, which is also its issuer.
///
/// A name, because X.509 requires one; nothing reads it. The node's real name is the key.
const SUBJECT: &[u8] = b"almena node";

/// The first instant the certificate claims to be valid at, as `UTCTime`.
///
/// `UTCTime` because X.509 requires that shape for any year before 2050, and a date is chosen
/// that predates every node there will ever be, so that a clock set wrongly to the past cannot
/// find the certificate not yet valid.
const NOT_BEFORE: &[u8] = b"250101000000Z";

/// The last instant, as `GeneralizedTime`.
///
/// The far end of the calendar, which is how X.509 spells *no expiry*, and `GeneralizedTime`
/// because that is the shape a year past 2049 has to take.
const NOT_AFTER: &[u8] = b"99991231235959Z";

/// The version number that means X.509 v3.
const V3: u8 = 2;

/// The version number that means PKCS#8 v1 — a private key with no public half beside it.
const PKCS8_V1: u8 = 0;

/// The certificate a node made from its own key, with the key beside it in the shape TLS takes.
///
/// Both halves are DER: the certificate as any client reads it, and the key as PKCS#8, which is
/// what the TLS implementation loads without being told what kind of key it is.
pub struct OwnKey {
    certificate: CertificateDer<'static>,
    key: PrivateKeyDer<'static>,
}

impl OwnKey {
    /// The certificate, which is what a client sees and pins.
    #[must_use]
    pub fn certificate(&self) -> &CertificateDer<'static> {
        &self.certificate
    }

    /// The two halves, for whatever serves under them.
    #[must_use]
    pub fn into_parts(self) -> (CertificateDer<'static>, PrivateKeyDer<'static>) {
        (self.certificate, self.key)
    }
}

/// The certificate for this key, signed by this key.
///
/// Deterministic: the same secret gives the same bytes every time, and nothing here reads a
/// clock or draws a number. What comes out is X.509 v3 with a serial taken from the key itself,
/// Ed25519 as both the key's algorithm and the signature's, issuer and subject both the fixed
/// name, the validity a formality, and no extensions — the whole shape a pin needs and nothing
/// a pin ignores.
#[must_use]
pub fn own_key(secret: &[u8; PUBLIC_KEY_WIDTH]) -> OwnKey {
    let signing = ed25519::SigningKey::from_secret(*secret);
    let public = signing.verifying_key().bytes();

    let to_be_signed = to_be_signed(&public);
    let signature = signing.sign(&to_be_signed).bytes();

    let mut certificate = to_be_signed;
    certificate.extend_from_slice(&algorithm());
    certificate.extend_from_slice(&bit_string(&signature));

    OwnKey {
        certificate: CertificateDer::from(element(tag::SEQUENCE, &certificate)),
        key: PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(pkcs8(secret))),
    }
}

/// The part of the certificate the signature is over.
fn to_be_signed(public: &[u8; PUBLIC_KEY_WIDTH]) -> Vec<u8> {
    let mut validity = element(tag::UTC_TIME, NOT_BEFORE);
    validity.extend_from_slice(&element(tag::GENERALIZED_TIME, NOT_AFTER));

    let mut body = element(tag::VERSION, &element(tag::INTEGER, &[V3]));
    // A serial has to be a positive integer and is meant to tell certificates from one issuer
    // apart. Every node is its own issuer, so any value would do; the key's own leading bytes are
    // taken so that the serial, like everything else here, is a function of the key.
    body.extend_from_slice(&element(tag::INTEGER, &positive(&public[..8])));
    body.extend_from_slice(&algorithm());
    body.extend_from_slice(&name());
    body.extend_from_slice(&element(tag::SEQUENCE, &validity));
    body.extend_from_slice(&name());
    body.extend_from_slice(&subject_public_key_info(public));
    element(tag::SEQUENCE, &body)
}

/// The key as PKCS#8 v1, which is the shape the TLS implementation loads.
///
/// The secret sits inside two octet strings: the outer one is PKCS#8's `privateKey` field, and
/// the inner one is how RFC 8410 says an Ed25519 key is written inside it.
fn pkcs8(secret: &[u8; PUBLIC_KEY_WIDTH]) -> Vec<u8> {
    let mut body = element(tag::INTEGER, &[PKCS8_V1]);
    body.extend_from_slice(&algorithm());
    body.extend_from_slice(&element(
        tag::OCTET_STRING,
        &element(tag::OCTET_STRING, secret),
    ));
    element(tag::SEQUENCE, &body)
}

/// The `AlgorithmIdentifier` for Ed25519: the identifier alone, with no parameters.
fn algorithm() -> Vec<u8> {
    element(tag::SEQUENCE, &ID_ED25519)
}

/// The one name, used for both issuer and subject: `CN=almena node`.
fn name() -> Vec<u8> {
    let mut attribute = COMMON_NAME.to_vec();
    attribute.extend_from_slice(&element(tag::UTF8_STRING, SUBJECT));
    element(
        tag::SEQUENCE,
        &element(tag::SET, &element(tag::SEQUENCE, &attribute)),
    )
}

/// The `SubjectPublicKeyInfo` carrying the node's key, which is the part a pin reads.
fn subject_public_key_info(public: &[u8; PUBLIC_KEY_WIDTH]) -> Vec<u8> {
    let mut body = algorithm();
    body.extend_from_slice(&bit_string(public));
    element(tag::SEQUENCE, &body)
}

/// A bit string whose every bit is used, which is how a key and a signature are wrapped.
///
/// The leading byte says how many bits of the last byte are padding; here none are.
fn bit_string(whole_bytes: &[u8]) -> Vec<u8> {
    let mut body = Vec::with_capacity(1 + whole_bytes.len());
    body.push(0);
    body.extend_from_slice(whole_bytes);
    element(tag::BIT_STRING, &body)
}

/// The bytes of a positive integer, written the one way DER allows.
///
/// DER admits exactly one encoding of a value: no leading zero byte unless the byte after it
/// would otherwise read as a sign bit, and a value of zero is one zero byte. A serial written any
/// other way is refused by a strict reader, and the strict readers are the ones worth reaching.
fn positive(magnitude: &[u8]) -> Vec<u8> {
    let significant = magnitude
        .iter()
        .position(|byte| *byte != 0)
        .map_or(&[][..], |first| &magnitude[first..]);
    match significant.first() {
        None => vec![0],
        Some(first) if first & 0x80 != 0 => {
            let mut signed = Vec::with_capacity(1 + significant.len());
            signed.push(0);
            signed.extend_from_slice(significant);
            signed
        }
        Some(_) => significant.to_vec(),
    }
}

/// One DER element: the tag, the length, the content.
///
/// Lengths under 128 take one byte; longer ones say how many bytes the length takes and then
/// give it, most significant first, with no leading zeros — which is the only form DER admits.
fn element(tag: u8, content: &[u8]) -> Vec<u8> {
    let mut written = Vec::with_capacity(content.len() + 6);
    written.push(tag);
    let length = content.len();
    if length < 0x80 {
        // A single byte carries the length, and `length < 0x80` is what makes the narrowing exact.
        #[allow(clippy::cast_possible_truncation)]
        written.push(length as u8);
    } else {
        let bytes = length.to_be_bytes();
        let significant = bytes
            .iter()
            .position(|byte| *byte != 0)
            .map_or(&[][..], |first| &bytes[first..]);
        // At most eight bytes, and the narrowing is exact for the same reason.
        #[allow(clippy::cast_possible_truncation)]
        written.push(0x80 | significant.len() as u8);
        written.extend_from_slice(significant);
    }
    written.extend_from_slice(content);
    written
}

/// Why no node key could be read out of a certificate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoKey {
    /// Not a certificate this reader can walk: not DER, or not the shape X.509 gives one.
    NotACertificate,
    /// A certificate, but the key in it is of another kind than a node's.
    ///
    /// Told apart from the first because it is the answer a pin most needs to be precise about: a
    /// certificate somebody obtained from an authority is a valid certificate for a server, and
    /// still not a node saying who it is.
    AnotherKind,
}

/// The Ed25519 key a certificate carries as its subject public key.
///
/// **This is the reference for whatever pins a node.** A client dialling a node was told the
/// node's key by the zone or by the record; it reads the key out of what the node presented with
/// this, compares the thirty-two bytes, and needs nothing else — not a name, not a chain, not a
/// date. Written to read any certificate of that shape and not only this crate's own, since an
/// operator may serve under one obtained elsewhere with the same key inside it.
///
/// # Errors
///
/// [`NoKey`], and which one says whether it is worth looking harder.
pub fn key_in(certificate: &[u8]) -> Result<[u8; PUBLIC_KEY_WIDTH], NoKey> {
    let (whole, trailing) = read(certificate, tag::SEQUENCE)?;
    if !trailing.is_empty() {
        return Err(NoKey::NotACertificate);
    }
    let (mut to_be_signed, _) = read(whole, tag::SEQUENCE)?;

    // The version is optional in the grammar and always present in a v3 certificate; a reader
    // that required it would refuse nothing worth refusing and one that ignored it would misread
    // the serial.
    if to_be_signed.first() == Some(&tag::VERSION) {
        to_be_signed = read(to_be_signed, tag::VERSION)?.1;
    }
    to_be_signed = read(to_be_signed, tag::INTEGER)?.1;
    for _ in ["signature algorithm", "issuer", "validity", "subject"] {
        to_be_signed = read(to_be_signed, tag::SEQUENCE)?.1;
    }
    let (info, _) = read(to_be_signed, tag::SEQUENCE)?;

    let (algorithm, after) = read(info, tag::SEQUENCE)?;
    if algorithm != ID_ED25519 {
        return Err(NoKey::AnotherKind);
    }
    let (bits, _) = read(after, tag::BIT_STRING)?;
    match bits.split_first() {
        Some((0, key)) => key.try_into().map_err(|_| NoKey::AnotherKind),
        _ => Err(NoKey::AnotherKind),
    }
}

/// One element off the front of `bytes`: its content, and what follows it.
///
/// Refuses a tag other than the one expected, a length that runs past the end, and the long
/// length forms nothing this size needs — which is a reader for certificates and not for DER in
/// general, on purpose.
fn read(bytes: &[u8], expected: u8) -> Result<(&[u8], &[u8]), NoKey> {
    let (&found, after_tag) = bytes.split_first().ok_or(NoKey::NotACertificate)?;
    if found != expected {
        return Err(NoKey::NotACertificate);
    }
    let (&first, after_length) = after_tag.split_first().ok_or(NoKey::NotACertificate)?;
    let (length, content) = if first < 0x80 {
        (usize::from(first), after_length)
    } else {
        let taking = usize::from(first & 0x7F);
        if taking == 0 || taking > 4 || after_length.len() < taking {
            return Err(NoKey::NotACertificate);
        }
        let (digits, content) = after_length.split_at(taking);
        let length = digits
            .iter()
            .fold(0usize, |so_far, digit| (so_far << 8) | usize::from(*digit));
        (length, content)
    };
    if content.len() < length {
        return Err(NoKey::NotACertificate);
    }
    Ok(content.split_at(length))
}

#[cfg(test)]
mod tests {
    use super::{ID_ED25519, NoKey, element, key_in, own_key, positive, read, tag};
    use almena_suite::ed25519::{Signature, SigningKey};
    use rustls_pki_types::PrivateKeyDer;

    const SECRET: [u8; 32] = [7; 32];

    #[test]
    fn the_key_in_the_certificate_is_the_node_key() {
        // The whole point: what a client reads out is what the zone told it to expect.
        let expected = SigningKey::from_secret(SECRET).verifying_key().bytes();
        let own = own_key(&SECRET);
        assert_eq!(key_in(own.certificate().as_ref()), Ok(expected));
    }

    #[test]
    fn the_certificate_is_signed_by_that_key_and_by_nobody_else() {
        // Checked with the suite and not with a TLS library, so that what is proved is the bytes
        // and not one implementation's tolerance for them: the signature sits over exactly the
        // first element inside the certificate.
        let own = own_key(&SECRET);
        let der = own.certificate().as_ref();
        let (whole, _) = read(der, tag::SEQUENCE).unwrap();
        let (_, after_body) = read(whole, tag::SEQUENCE).unwrap();
        let body_length = whole.len() - after_body.len();
        let to_be_signed = &whole[..body_length];

        let (_, after_algorithm) = read(after_body, tag::SEQUENCE).unwrap();
        let (bits, trailing) = read(after_algorithm, tag::BIT_STRING).unwrap();
        assert!(trailing.is_empty(), "nothing follows the signature");
        let (&padding, signature) = bits.split_first().unwrap();
        assert_eq!(padding, 0);
        let signature = Signature::from_bytes(signature.try_into().unwrap());

        let signing = SigningKey::from_secret(SECRET);
        assert_eq!(
            signing.verifying_key().verify(to_be_signed, &signature),
            Ok(())
        );
        let stranger = SigningKey::from_secret([8; 32]);
        assert!(
            stranger
                .verifying_key()
                .verify(to_be_signed, &signature)
                .is_err()
        );
    }

    #[test]
    fn the_same_key_gives_the_same_bytes_every_time() {
        // No clock and no randomness: a node that restarts presents what it presented before,
        // and a test can hold the bytes against a vector.
        assert_eq!(
            own_key(&SECRET).certificate().as_ref(),
            own_key(&SECRET).certificate().as_ref()
        );
        assert_ne!(
            own_key(&SECRET).certificate().as_ref(),
            own_key(&[8; 32]).certificate().as_ref()
        );
    }

    #[test]
    fn the_private_half_is_pkcs8_and_the_types_crate_recognises_it_as_such() {
        // What comes out is handed to a TLS implementation that decides what a key is by its
        // shape. The bytes are read back through the same types crate that reads a file, and
        // they come back as the same kind.
        let (_, key) = own_key(&SECRET).into_parts();
        let PrivateKeyDer::Pkcs8(pkcs8) = &key else {
            panic!("PKCS#8, which is the one shape that names its own algorithm");
        };
        let bytes = pkcs8.secret_pkcs8_der().to_vec();
        assert!(matches!(
            PrivateKeyDer::try_from(bytes.as_slice()),
            Ok(PrivateKeyDer::Pkcs8(again)) if again.secret_pkcs8_der() == bytes
        ));
        assert!(
            bytes.windows(SECRET.len()).any(|window| window == SECRET),
            "and the secret is inside it, where the implementation looks"
        );
    }

    #[test]
    fn a_certificate_with_another_kind_of_key_is_said_to_be_one() {
        // The answer a pin most needs precision on: a valid certificate that is not a node's.
        let own = own_key(&SECRET);
        let mut der = own.certificate().as_ref().to_vec();
        // Ed448 is `1.3.101.113`, one arc along from Ed25519; the shape stays a certificate.
        let ed448 = [tag::OBJECT_IDENTIFIER, 0x03, 0x2B, 0x65, 0x71];
        let mut at = 0;
        while let Some(found) = der[at..]
            .windows(ID_ED25519.len())
            .position(|window| window == ID_ED25519)
        {
            der[at + found..at + found + ed448.len()].copy_from_slice(&ed448);
            at += found + ed448.len();
        }
        assert_eq!(key_in(&der), Err(NoKey::AnotherKind));
    }

    #[test]
    fn something_that_is_not_a_certificate_is_refused_and_not_guessed_at() {
        for not_one in [
            &b""[..],
            b"-----BEGIN CERTIFICATE-----",
            &[0x30],
            &[0x30, 0x05, 0x01],
            &[0x30, 0x84, 0xFF, 0xFF, 0xFF, 0xFF],
            &[0x02, 0x01, 0x01],
        ] {
            assert_eq!(key_in(not_one), Err(NoKey::NotACertificate), "{not_one:?}");
        }

        // A byte too many after a well-formed certificate is also not a certificate: a reader
        // that took the first one and ignored the rest would let something ride behind it.
        let own = own_key(&SECRET);
        let mut trailing = own.certificate().as_ref().to_vec();
        trailing.push(0);
        assert_eq!(key_in(&trailing), Err(NoKey::NotACertificate));
    }

    #[test]
    fn a_length_is_written_the_one_way_der_allows() {
        assert_eq!(element(0x04, &[1, 2, 3]), vec![0x04, 3, 1, 2, 3]);
        assert_eq!(element(0x04, &[0; 127])[..2], [0x04, 0x7F]);
        assert_eq!(element(0x04, &[0; 128])[..3], [0x04, 0x81, 0x80]);
        assert_eq!(element(0x04, &[0; 300])[..4], [0x04, 0x82, 0x01, 0x2C]);

        // And read back, whichever form it took.
        for size in [0, 1, 127, 128, 255, 256, 300] {
            let written = element(0x04, &vec![9; size]);
            let (content, rest) = read(&written, 0x04).unwrap();
            assert_eq!(content.len(), size);
            assert!(rest.is_empty());
        }
    }

    #[test]
    fn a_serial_is_a_positive_integer_in_its_shortest_form() {
        // No leading zero unless the sign bit would be misread, and zero is one byte.
        assert_eq!(positive(&[0, 0, 0]), vec![0]);
        assert_eq!(positive(&[0, 0, 5]), vec![5]);
        assert_eq!(positive(&[0x7F, 1]), vec![0x7F, 1]);
        assert_eq!(positive(&[0x80, 1]), vec![0, 0x80, 1]);
        assert_eq!(positive(&[0, 0x80]), vec![0, 0x80]);
    }
}
