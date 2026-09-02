//! The two pieces a DIDComm envelope is built out of, each checked against a published vector.
//!
//! **Neither of these is this project's invention, and neither is tested only against itself.** A
//! derivation or a cipher that round-trips with its own implementation and nothing else can be
//! confidently, symmetrically wrong — so what these are held to is the numbers in the standards
//! that define them, which the holder's app is running a different implementation of.
//!
//! The cipher is written out here over the block cipher rather than taken from a mode crate, for
//! the reason the key wrap is: CBC with PKCS#7 padding is a page of arithmetic, and the vector in
//! RFC 7518 is what says the page is right.

use aes::Aes256;
use aes::cipher::generic_array::GenericArray;
use aes::cipher::{BlockDecrypt as _, BlockEncrypt as _, KeyInit as _};
use hmac::Mac as _;

/// How long the key a message is sealed with is.
pub const KEY_WIDTH: usize = 32;

/// How long the tag on a sealed message is: half the digest, which for SHA-512 is thirty-two.
const TAG_WIDTH: usize = 32;

/// How long the block the body is sealed in blocks of is.
const BLOCK: usize = 16;

/// What a derived key is derived *for*, every part of it length-prefixed into the key.
#[derive(Debug, Clone, Copy)]
pub struct About<'a> {
    /// What the key is for, named the way JOSE names algorithms.
    pub algorithm: &'a str,
    /// Who is sending.
    pub sender: &'a [u8],
    /// Who is receiving.
    pub recipient: &'a [u8],
    /// What binds this derivation to this one message: the tag, for the authenticated form.
    pub tag: &'a [u8],
}

/// Derive a key of `wanted` bytes from a shared secret, the way JOSE derives one.
///
/// **Concat KDF** (NIST SP 800-56A §5.8.1). Every field is length-prefixed, which is the whole of
/// why it is safe to concatenate them.
#[must_use]
pub fn derived(secret: &[u8], about: &About<'_>, wanted: usize) -> Vec<u8> {
    use sha2::Digest as _;
    let mut out = Vec::with_capacity(wanted);
    let mut counter: u32 = 1;
    while out.len() < wanted {
        let mut hash = sha2::Sha256::new();
        hash.update(counter.to_be_bytes());
        hash.update(secret);
        for part in [about.algorithm.as_bytes(), about.sender, about.recipient] {
            hash.update(u32::try_from(part.len()).unwrap_or(u32::MAX).to_be_bytes());
            hash.update(part);
        }
        // The width asked for, in bits, so that a key of one length is not a prefix of another.
        hash.update(u32::try_from(wanted * 8).unwrap_or(u32::MAX).to_be_bytes());
        hash.update(about.tag);
        out.extend_from_slice(&hash.finalize());
        counter += 1;
    }
    out.truncate(wanted);
    out
}

/// Something did not seal, or did not open.
///
/// **One error for everything**, deliberately: telling a caller whether the key was the wrong
/// width, the tag failed or the padding did is telling whoever composed the message which half to
/// work on next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotSealed;

/// A body sealed with `A256CBC-HS512`, and the tag over it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sealed {
    /// The body, as blocks.
    pub bytes: Vec<u8>,
    /// What holds the body and the header together.
    pub tag: Vec<u8>,
}

/// Seal a body, authenticating the header along with it.
///
/// `key` is sixty-four bytes: **the first half signs and the second half encrypts**, in that
/// order, which is the order the standard fixes and the one place this is easy to get backwards.
///
/// # Errors
///
/// [`NotSealed`], for a key that is not sixty-four bytes or an initial value that is not sixteen.
pub fn seal(key: &[u8], iv: &[u8], body: &[u8], header: &[u8]) -> Result<Sealed, NotSealed> {
    let (signs, seals) = halves(key)?;
    let iv: [u8; BLOCK] = iv.try_into().map_err(|_| NotSealed)?;
    let bytes = cbc_encrypt(seals, &iv, body);
    Ok(Sealed {
        tag: held(signs, header, &iv, &bytes),
        bytes,
    })
}

/// Open one, and refuse it whole if the tag does not hold.
///
/// # Errors
///
/// [`NotSealed`], one error and not several.
pub fn open(key: &[u8], iv: &[u8], sealed: &Sealed, header: &[u8]) -> Result<Vec<u8>, NotSealed> {
    let (signs, seals) = halves(key)?;
    let iv: [u8; BLOCK] = iv.try_into().map_err(|_| NotSealed)?;
    // **Checked before anything is decrypted.** Padding read before the tag is checked is an
    // oracle: whoever sent it learns something from how it failed, one question at a time.
    let ours = held(signs, header, &iv, &sealed.bytes);
    if !same(&ours, &sealed.tag) {
        return Err(NotSealed);
    }
    cbc_decrypt(seals, &iv, &sealed.bytes)
}

/// The two halves of the key, in the order the standard fixes.
fn halves(key: &[u8]) -> Result<(&[u8], &[u8]), NotSealed> {
    if key.len() != KEY_WIDTH * 2 {
        return Err(NotSealed);
    }
    Ok(key.split_at(KEY_WIDTH))
}

/// CBC over AES-256 with PKCS#7 padding, which always adds between one and sixteen bytes.
fn cbc_encrypt(key: &[u8], iv: &[u8; BLOCK], body: &[u8]) -> Vec<u8> {
    let cipher = Aes256::new(GenericArray::from_slice(key));
    let pad = BLOCK - body.len() % BLOCK;
    let mut padded = body.to_vec();
    padded.extend(std::iter::repeat_n(pad as u8, pad));
    let mut previous = *iv;
    for block in padded.chunks_mut(BLOCK) {
        for (byte, chained) in block.iter_mut().zip(previous) {
            *byte ^= chained;
        }
        cipher.encrypt_block(GenericArray::from_mut_slice(block));
        previous.copy_from_slice(block);
    }
    padded
}

/// The inverse, with the padding checked and stripped.
fn cbc_decrypt(key: &[u8], iv: &[u8; BLOCK], bytes: &[u8]) -> Result<Vec<u8>, NotSealed> {
    if bytes.is_empty() || !bytes.len().is_multiple_of(BLOCK) {
        return Err(NotSealed);
    }
    let cipher = Aes256::new(GenericArray::from_slice(key));
    let mut out = bytes.to_vec();
    let mut previous = *iv;
    for block in out.chunks_mut(BLOCK) {
        let sealed: [u8; BLOCK] = block.try_into().map_err(|_| NotSealed)?;
        cipher.decrypt_block(GenericArray::from_mut_slice(block));
        for (byte, chained) in block.iter_mut().zip(previous) {
            *byte ^= chained;
        }
        previous = sealed;
    }
    let pad = usize::from(*out.last().ok_or(NotSealed)?);
    if pad == 0 || pad > BLOCK || pad > out.len() {
        return Err(NotSealed);
    }
    if !out[out.len() - pad..]
        .iter()
        .all(|byte| usize::from(*byte) == pad)
    {
        return Err(NotSealed);
    }
    out.truncate(out.len() - pad);
    Ok(out)
}

/// What holds a header and a body together, truncated as the standard says.
fn held(signs: &[u8], header: &[u8], iv: &[u8], bytes: &[u8]) -> Vec<u8> {
    // HMAC takes a key of any length, so the one error it has cannot happen here; an empty tag
    // rather than a panic if it ever did, which `open` then refuses.
    let Ok(mut mac) = <hmac::Hmac<sha2::Sha512> as hmac::Mac>::new_from_slice(signs) else {
        return Vec::new();
    };
    mac.update(header);
    mac.update(iv);
    mac.update(bytes);
    // The length of the header in **bits**, as sixty-four bits: without it, moving bytes from the
    // end of the header into the start of the body would leave the tag unchanged.
    mac.update(&(header.len() as u64 * 8).to_be_bytes());
    mac.finalize().into_bytes()[..TAG_WIDTH].to_vec()
}

/// Whether two tags are the same, in time that does not depend on where they differ.
fn same(ours: &[u8], theirs: &[u8]) -> bool {
    if ours.len() != theirs.len() {
        return false;
    }
    ours.iter()
        .zip(theirs)
        .fold(0u8, |sofar, (one, other)| sofar | (one ^ other))
        == 0
}

#[cfg(test)]
mod tests {
    use super::{About, NotSealed, Sealed, derived, open, seal};

    #[test]
    fn the_derivation_agrees_with_the_number_in_the_standard() {
        // **RFC 7518 §C**, the worked example every JOSE implementation is checked against.
        let secret = [
            158, 86, 217, 29, 129, 113, 53, 211, 114, 131, 66, 131, 191, 132, 38, 156, 251, 49,
            110, 163, 218, 128, 106, 72, 246, 218, 167, 121, 140, 254, 144, 196,
        ];
        assert_eq!(
            derived(
                &secret,
                &About {
                    algorithm: "A128GCM",
                    sender: b"Alice",
                    recipient: b"Bob",
                    tag: &[],
                },
                16,
            ),
            [
                86, 170, 141, 234, 248, 35, 109, 32, 92, 34, 40, 205, 113, 167, 16, 26
            ]
        );
    }

    #[test]
    fn the_cipher_agrees_with_the_numbers_in_the_standard() {
        // **RFC 7518 §B.3**, the `A256CBC-HS512` worked example, copied out of the document and
        // out of the holder's app rather than remembered. Both the bytes and the tag.
        let key: Vec<u8> = (0x00u8..=0x3f).collect();
        let iv: [u8; 16] = [
            0x1a, 0xf3, 0x8c, 0x2d, 0xc2, 0xb9, 0x6f, 0xfd, 0xd8, 0x66, 0x94, 0x09, 0x23, 0x41,
            0xbc, 0x04,
        ];
        let body = b"A cipher system must not be required to be secret, and it must be able to fall into the hands of the enemy without inconvenience";
        let header = b"The second principle of Auguste Kerckhoffs";
        let expected: [u8; 144] = [
            0x4a, 0xff, 0xaa, 0xad, 0xb7, 0x8c, 0x31, 0xc5, 0xda, 0x4b, 0x1b, 0x59, 0x0d, 0x10,
            0xff, 0xbd, 0x3d, 0xd8, 0xd5, 0xd3, 0x02, 0x42, 0x35, 0x26, 0x91, 0x2d, 0xa0, 0x37,
            0xec, 0xbc, 0xc7, 0xbd, 0x82, 0x2c, 0x30, 0x1d, 0xd6, 0x7c, 0x37, 0x3b, 0xcc, 0xb5,
            0x84, 0xad, 0x3e, 0x92, 0x79, 0xc2, 0xe6, 0xd1, 0x2a, 0x13, 0x74, 0xb7, 0x7f, 0x07,
            0x75, 0x53, 0xdf, 0x82, 0x94, 0x10, 0x44, 0x6b, 0x36, 0xeb, 0xd9, 0x70, 0x66, 0x29,
            0x6a, 0xe6, 0x42, 0x7e, 0xa7, 0x5c, 0x2e, 0x08, 0x46, 0xa1, 0x1a, 0x09, 0xcc, 0xf5,
            0x37, 0x0d, 0xc8, 0x0b, 0xfe, 0xcb, 0xad, 0x28, 0xc7, 0x3f, 0x09, 0xb3, 0xa3, 0xb7,
            0x5e, 0x66, 0x2a, 0x25, 0x94, 0x41, 0x0a, 0xe4, 0x96, 0xb2, 0xe2, 0xe6, 0x60, 0x9e,
            0x31, 0xe6, 0xe0, 0x2c, 0xc8, 0x37, 0xf0, 0x53, 0xd2, 0x1f, 0x37, 0xff, 0x4f, 0x51,
            0x95, 0x0b, 0xbe, 0x26, 0x38, 0xd0, 0x9d, 0xd7, 0xa4, 0x93, 0x09, 0x30, 0x80, 0x6d,
            0x07, 0x03, 0xb1, 0xf6,
        ];
        let tag: [u8; 32] = [
            0x4d, 0xd3, 0xb4, 0xc0, 0x88, 0xa7, 0xf4, 0x5c, 0x21, 0x68, 0x39, 0x64, 0x5b, 0x20,
            0x12, 0xbf, 0x2e, 0x62, 0x69, 0xa8, 0xc5, 0x6a, 0x81, 0x6d, 0xbc, 0x1b, 0x26, 0x77,
            0x61, 0x95, 0x5b, 0xc5,
        ];
        assert_eq!(body.len(), 128, "the standard's own body");
        let sealed = seal(&key, &iv, body, header).expect("the standard's own sizes");
        assert_eq!(sealed.bytes, expected);
        assert_eq!(sealed.tag, tag);
        assert_eq!(open(&key, &iv, &sealed, header), Ok(body.to_vec()));
    }

    #[test]
    fn a_body_whose_header_or_tag_was_changed_does_not_open() {
        let key: Vec<u8> = (0x00u8..=0x3f).collect();
        let iv = [7u8; 16];
        let sealed = seal(&key, &iv, b"the message", b"the header").expect("sizes");
        assert_eq!(open(&key, &iv, &sealed, b"the headeR"), Err(NotSealed));
        let mut spoiled = sealed.clone();
        spoiled.tag[0] ^= 1;
        assert_eq!(open(&key, &iv, &spoiled, b"the header"), Err(NotSealed));
        assert_eq!(
            open(
                &key,
                &iv,
                &Sealed {
                    bytes: vec![0; 32],
                    tag: vec![0; 16]
                },
                b"the header"
            ),
            Err(NotSealed)
        );
    }

    #[test]
    fn every_length_of_body_pads_and_unpads_to_itself() {
        let key: Vec<u8> = (0x00u8..=0x3f).collect();
        let iv = [3u8; 16];
        for length in [0usize, 1, 15, 16, 17, 31, 32, 100] {
            let body = vec![0x41u8; length];
            let sealed = seal(&key, &iv, &body, b"h").expect("sizes");
            assert_eq!(sealed.bytes.len() % 16, 0);
            assert!(sealed.bytes.len() > length);
            assert_eq!(open(&key, &iv, &sealed, b"h"), Ok(body), "{length}");
        }
    }

    #[test]
    fn a_key_that_is_not_the_right_size_is_refused_rather_than_stretched() {
        assert_eq!(seal(&[0; 32], &[0; 16], b"body", b"head"), Err(NotSealed));
        assert_eq!(seal(&[0; 64], &[0; 12], b"body", b"head"), Err(NotSealed));
    }
}
