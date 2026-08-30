//! Base64url without padding, written here rather than depended on.
//!
//! # Why this is not a dependency
//!
//! **Because decoding has to be strict, and a general-purpose decoder's job is to be lenient.**
//! Every byte string in this crate is either hashed or signed over: a disclosure's digest is taken
//! over its base64url text, and a JWS signature is taken over `header.payload`. So two spellings of
//! one value are two digests of one claim, and a decoder that accepted padding, or an alphabet it
//! was not given, or trailing bits that encode nothing, would let one disclosure have two names.
//!
//! It is the same argument the canonical CBOR profile makes and the same one `identifier.rs` makes
//! for base58: an encoding that decides a name is part of the format, and belongs where the format
//! is written down.
//!
//! **No padding**, because JOSE has none: RFC 7515 §2 defines base64url encoding for JOSE with the
//! trailing `=` removed, and a string carrying them is not one this reads.

/// The alphabet, in the one order it may be written in.
const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// Why some text is not base64url.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotBase64Url {
    /// A character outside the alphabet — including `=`, `+` and `/`.
    Character,
    /// A length no base64url string has: one leftover character encodes nothing.
    Length,
    /// The final character carries bits beyond the bytes it encodes.
    ///
    /// **Refused rather than masked off.** Two spellings of one byte string would be two digests of
    /// one disclosure, and a reader that quietly dropped the extra bits would accept both.
    NotCanonical,
}

/// Those bytes, written the one way this platform writes them.
#[must_use]
pub fn encode(bytes: &[u8]) -> String {
    let mut held = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for group in bytes.chunks(3) {
        let mut packed = 0u32;
        for (at, byte) in group.iter().enumerate() {
            packed |= u32::from(*byte) << (16 - at * 8);
        }
        // One character per six bits, and one fewer group of six for every byte the chunk is short.
        for at in 0..=group.len() {
            let six = (packed >> (18 - at * 6)) & 0x3f;
            held.push(char::from(ALPHABET[six as usize]));
        }
    }
    held
}

/// The bytes that text stands for.
///
/// # Errors
///
/// [`NotBase64Url`], telling apart a character that is not in the alphabet from a length no
/// base64url string has and from one whose last character carries bits beyond the bytes.
pub fn decode(text: &str) -> Result<Vec<u8>, NotBase64Url> {
    let mut bytes = Vec::with_capacity(text.len() / 4 * 3);
    for group in text.as_bytes().chunks(4) {
        // One character is a group that encodes nothing at all: six bits are not a byte.
        if group.len() == 1 {
            return Err(NotBase64Url::Length);
        }
        let mut packed = 0u32;
        for (at, character) in group.iter().enumerate() {
            let six = ALPHABET
                .iter()
                .position(|held| held == character)
                .ok_or(NotBase64Url::Character)?;
            packed |= (six as u32) << (18 - at * 6);
        }
        for at in 0..group.len() - 1 {
            bytes.push(((packed >> (16 - at * 8)) & 0xff) as u8);
        }
        // What is left over in the last group has to be nought, or the same bytes have two
        // spellings and a digest over the text stops naming the value.
        let spare = (group.len() - 1) * 8;
        if packed & ((1 << (24 - spare)) - 1) != 0 {
            return Err(NotBase64Url::NotCanonical);
        }
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::{NotBase64Url, decode, encode};

    #[test]
    fn what_is_written_is_what_this_reads_and_nothing_else_is() {
        for held in [
            &b""[..],
            b"f",
            b"fo",
            b"foo",
            b"foob",
            b"fooba",
            b"foobar",
            &[0u8; 32],
            &[0xff; 33],
        ] {
            assert_eq!(decode(&encode(held)), Ok(held.to_vec()), "{held:?}");
        }
    }

    #[test]
    fn the_alphabet_is_the_url_one_and_there_is_no_padding() {
        // RFC 7515 §2: JOSE strips the padding, and uses `-` and `_` where the classic alphabet
        // uses `+` and `/`. A string with any of the three in it is not one this reads.
        assert_eq!(encode(&[0xfb, 0xff]), "-_8");
        assert_eq!(decode("-_8"), Ok(vec![0xfb, 0xff]));
        assert_eq!(decode("+/8"), Err(NotBase64Url::Character));
        assert_eq!(decode("-_8="), Err(NotBase64Url::Character));
    }

    #[test]
    fn a_spelling_that_carries_bits_beyond_its_bytes_is_refused() {
        // **Two spellings of one value would be two digests of one disclosure.** `QQ` is `A`;
        // `QR` decodes to the same byte with two bits left over, and a lenient reader takes both.
        assert_eq!(decode("QQ"), Ok(vec![0x41]));
        assert_eq!(decode("QR"), Err(NotBase64Url::NotCanonical));
        assert_eq!(decode("AAA"), Ok(vec![0, 0]));
        assert_eq!(decode("AAB"), Err(NotBase64Url::NotCanonical));
    }

    #[test]
    fn one_leftover_character_is_a_length_no_string_has() {
        assert_eq!(decode("A"), Err(NotBase64Url::Length));
        assert_eq!(decode("AAAAA"), Err(NotBase64Url::Length));
    }
}
