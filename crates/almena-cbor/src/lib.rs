//! What *canonical* means here, and the check that says whether some bytes are.
//!
//! Almena encodes in deterministic CBOR (RFC 8949 §4.2) with a profile of its own, and makes
//! the identifier of an object **the hash of those bytes**. Put together, those two decisions
//! mean something this crate exists to enforce: if two byte strings could ever carry the same
//! content, they would be two names for one thing, and the whole scheme of naming by hash
//! comes apart.
//!
//! So canonicity is not a nicety of the encoder. **It is a validity rule, checked on the way
//! in.** Bytes that are not canonical are refused rather than re-encoded — re-encoding would
//! break the signature over them, which is why a receiver keeps the original bytes and
//! verifies against them. Canonicalising is the sender's obligation and nobody else's.
//!
//! # The profile
//!
//! - **No floats.** Everything is an integer; anything fractional is a scaled integer or a
//!   string. Floats are where determinism goes to die.
//! - **No indefinite lengths.** Every string, array and map declares its length up front.
//! - **Shortest form.** A value that fits in the head is written in the head; one that fits in
//!   one byte is not written in two.
//! - **Map keys sorted by their encoded bytes**, and no key twice.
//! - **Tags only from a whitelist**, which is empty today — see [`Violation::Tag`].
//! - **Text is valid UTF-8**, as CBOR requires and nothing here assumes.
//!
//! # What this crate does not do
//!
//! It does not encode. Encoding a typed operation is the business of whatever owns that type,
//! and this crate is what that code asserts against before handing bytes to a hash. Keeping the
//! check separate from the writer is deliberate: a validator that only ever saw its own
//! encoder's output would be a validator that agrees with a bug.
//!
//! # Replicated, and held to the same vectors
//!
//! `client` carries its own copy of this profile, because the repositories share no code: a
//! common crate would make one project the owner of everybody else's format and move the
//! ground under them with its versions. What keeps the two from drifting is the golden
//! vectors, not this file.

/// A rule of the profile that some bytes broke.
///
/// Each variant names the rule rather than the byte, because the byte is only interesting once
/// you know which promise it failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Violation {
    /// The input ended in the middle of an item.
    Truncated,
    /// The input carried more bytes after a complete item. One item, exactly.
    Trailing,
    /// A length or value was written in more bytes than it needed.
    NotShortest,
    /// A string, array or map declared no length. The profile has no indefinite lengths.
    IndefiniteLength,
    /// An additional-information value RFC 8949 reserves and gives no meaning.
    Reserved,
    /// A float. The profile has none, at any width.
    Float,
    /// A simple value other than `false`, `true` and `null`.
    Simple,
    /// A tag. The whitelist is empty today: a tag is a meaning the format has not agreed on,
    /// and agreeing on one is a change to the format itself before it is a change here.
    Tag,
    /// A text string that is not valid UTF-8.
    NotUtf8,
    /// Two map keys out of order, or the same key twice — told apart by which is which is not
    /// worth a variant, because both mean the same thing: the map is not canonical.
    MapOrder,
    /// A length that does not fit this machine's address space. Not a malformed input so much
    /// as one this build cannot hold.
    TooLarge,
}

/// Whether these bytes are exactly one canonical CBOR item under the profile.
///
/// # Errors
///
/// Returns the first [`Violation`] found. First and not all of them: the caller's next move is
/// the same either way, and a list would suggest a repair this crate does not offer.
pub fn canonical(bytes: &[u8]) -> Result<(), Violation> {
    let mut reader = Reader { bytes, at: 0 };
    reader.item()?;
    if reader.at == bytes.len() {
        Ok(())
    } else {
        Err(Violation::Trailing)
    }
}

/// Major type 7, which carries simple values and floats rather than an integer argument.
const SIMPLE_OR_FLOAT: u8 = 7;

/// A position in a byte string, and the walk over it.
struct Reader<'a> {
    /// The bytes being read.
    bytes: &'a [u8],
    /// How far in the walk has got.
    at: usize,
}

impl Reader<'_> {
    /// The next byte, advancing.
    fn byte(&mut self) -> Result<u8, Violation> {
        let byte = *self.bytes.get(self.at).ok_or(Violation::Truncated)?;
        self.at += 1;
        Ok(byte)
    }

    /// The next `count` bytes as an unsigned argument, rejecting any that had a shorter form.
    fn argument(&mut self, count: usize) -> Result<u64, Violation> {
        let mut value: u64 = 0;
        for _ in 0..count {
            value = (value << 8) | u64::from(self.byte()?);
        }
        let shortest = match count {
            1 => value >= 24,
            2 => value > u64::from(u8::MAX),
            4 => value > u64::from(u16::MAX),
            _ => value > u64::from(u32::MAX),
        };
        if shortest {
            Ok(value)
        } else {
            Err(Violation::NotShortest)
        }
    }

    /// The head of an item: its major type and its argument.
    fn head(&mut self) -> Result<(u8, u64), Violation> {
        let initial = self.byte()?;
        let major = initial >> 5;
        let info = initial & 0x1f;
        if major == SIMPLE_OR_FLOAT {
            return simple(info).map(|()| (major, u64::from(info)));
        }
        let argument = match info {
            0..=23 => u64::from(info),
            24 => self.argument(1)?,
            25 => self.argument(2)?,
            26 => self.argument(4)?,
            27 => self.argument(8)?,
            31 => return Err(Violation::IndefiniteLength),
            _ => return Err(Violation::Reserved),
        };
        Ok((major, argument))
    }

    /// One item, leaving the walk just past it.
    fn item(&mut self) -> Result<(), Violation> {
        let (major, argument) = self.head()?;
        match major {
            0 | 1 | SIMPLE_OR_FLOAT => Ok(()),
            2 => self.skip(argument).map(|_| ()),
            3 => self.text(argument),
            4 => self.sequence(argument, 1),
            5 => self.map(argument),
            _ => Err(Violation::Tag),
        }
    }

    /// `count` bytes of payload, returned so a caller can look at them.
    fn skip(&mut self, count: u64) -> Result<&'_ [u8], Violation> {
        let count = usize::try_from(count).map_err(|_| Violation::TooLarge)?;
        let end = self.at.checked_add(count).ok_or(Violation::TooLarge)?;
        let slice = self.bytes.get(self.at..end).ok_or(Violation::Truncated)?;
        self.at = end;
        Ok(slice)
    }

    /// A text string, which CBOR requires to be valid UTF-8.
    fn text(&mut self, length: u64) -> Result<(), Violation> {
        let slice = self.skip(length)?;
        core::str::from_utf8(slice)
            .map(|_| ())
            .map_err(|_| Violation::NotUtf8)
    }

    /// `count` groups of `per_group` items — an array with one, a map's contents with two.
    fn sequence(&mut self, count: u64, per_group: u64) -> Result<(), Violation> {
        let items = count.checked_mul(per_group).ok_or(Violation::TooLarge)?;
        for _ in 0..items {
            self.item()?;
        }
        Ok(())
    }

    /// A map, whose keys must rise strictly in the order of their encoded bytes.
    ///
    /// Strictly, so that the same check catches both a pair out of order and a key written
    /// twice: canonical order has no room for either, and the caller's answer to both is that
    /// these bytes are not the bytes anybody agreed on.
    fn map(&mut self, pairs: u64) -> Result<(), Violation> {
        let mut previous: Option<&[u8]> = None;
        for _ in 0..pairs {
            let from = self.at;
            self.item()?;
            let key = self.bytes.get(from..self.at).ok_or(Violation::Truncated)?;
            if previous.is_some_and(|earlier| earlier >= key) {
                return Err(Violation::MapOrder);
            }
            previous = Some(key);
            self.item()?;
        }
        Ok(())
    }
}

/// Whether an additional-information value of major type 7 is one the profile allows.
///
/// `false`, `true` and `null` and nothing else: `undefined` is a value with no meaning here,
/// a simple value in a following byte is a vocabulary nobody agreed on, and 25, 26 and 27 are
/// the three float widths the profile exists to keep out.
const fn simple(info: u8) -> Result<(), Violation> {
    match info {
        20..=22 => Ok(()),
        25..=27 => Err(Violation::Float),
        31 => Err(Violation::IndefiniteLength),
        28..=30 => Err(Violation::Reserved),
        _ => Err(Violation::Simple),
    }
}

#[cfg(test)]
mod tests {
    use super::{Violation, canonical};

    #[test]
    fn the_small_things_are_canonical() {
        assert_eq!(canonical(&[0x00]), Ok(()), "0");
        assert_eq!(canonical(&[0x17]), Ok(()), "23, the largest inline value");
        assert_eq!(
            canonical(&[0x18, 0x18]),
            Ok(()),
            "24, the smallest that needs a byte"
        );
        assert_eq!(canonical(&[0x20]), Ok(()), "-1");
        assert_eq!(canonical(&[0xf4]), Ok(()), "false");
        assert_eq!(canonical(&[0xf5]), Ok(()), "true");
        assert_eq!(canonical(&[0xf6]), Ok(()), "null");
        assert_eq!(canonical(&[0x40]), Ok(()), "an empty byte string");
        assert_eq!(canonical(&[0x60]), Ok(()), "an empty text string");
        assert_eq!(canonical(&[0x80]), Ok(()), "an empty array");
        assert_eq!(canonical(&[0xa0]), Ok(()), "an empty map");
    }

    #[test]
    fn a_value_written_longer_than_it_needs_is_refused() {
        assert_eq!(
            canonical(&[0x18, 0x17]),
            Err(Violation::NotShortest),
            "23 in two bytes"
        );
        assert_eq!(
            canonical(&[0x19, 0x00, 0xff]),
            Err(Violation::NotShortest),
            "255 in three"
        );
        assert_eq!(
            canonical(&[0x1a, 0x00, 0x00, 0xff, 0xff]),
            Err(Violation::NotShortest),
            "65535 in five"
        );
    }

    #[test]
    fn there_are_no_floats_at_any_width() {
        assert_eq!(
            canonical(&[0xf9, 0x00, 0x00]),
            Err(Violation::Float),
            "half"
        );
        assert_eq!(
            canonical(&[0xfa, 0, 0, 0, 0]),
            Err(Violation::Float),
            "single"
        );
        assert_eq!(
            canonical(&[0xfb, 0, 0, 0, 0, 0, 0, 0, 0]),
            Err(Violation::Float),
            "double"
        );
    }

    #[test]
    fn nothing_declines_to_state_its_length() {
        assert_eq!(
            canonical(&[0x5f, 0xff]),
            Err(Violation::IndefiniteLength),
            "bytes"
        );
        assert_eq!(
            canonical(&[0x7f, 0xff]),
            Err(Violation::IndefiniteLength),
            "text"
        );
        assert_eq!(
            canonical(&[0x9f, 0xff]),
            Err(Violation::IndefiniteLength),
            "array"
        );
        assert_eq!(
            canonical(&[0xbf, 0xff]),
            Err(Violation::IndefiniteLength),
            "map"
        );
    }

    #[test]
    fn a_tag_is_a_meaning_nobody_agreed_on() {
        assert_eq!(canonical(&[0xc0, 0x00]), Err(Violation::Tag));
    }

    #[test]
    fn undefined_and_unassigned_simple_values_are_refused() {
        assert_eq!(canonical(&[0xf7]), Err(Violation::Simple), "undefined");
        assert_eq!(canonical(&[0xf0]), Err(Violation::Simple), "simple(16)");
    }

    #[test]
    fn map_keys_rise_and_never_repeat() {
        // {1: 0, 2: 0}
        assert_eq!(canonical(&[0xa2, 0x01, 0x00, 0x02, 0x00]), Ok(()));
        // {2: 0, 1: 0} — out of order
        assert_eq!(
            canonical(&[0xa2, 0x02, 0x00, 0x01, 0x00]),
            Err(Violation::MapOrder)
        );
        // {1: 0, 1: 0} — the same key twice
        assert_eq!(
            canonical(&[0xa2, 0x01, 0x00, 0x01, 0x00]),
            Err(Violation::MapOrder)
        );
    }

    #[test]
    fn keys_are_compared_as_bytes_and_not_as_numbers() {
        // {23: 0, 24: 0} — 0x17 then 0x1818: shorter key first, and its bytes sort first too.
        assert_eq!(canonical(&[0xa2, 0x17, 0x00, 0x18, 0x18, 0x00]), Ok(()));
    }

    #[test]
    fn nesting_is_walked_through() {
        // [1, [2, 3], {1: "a"}]
        let nested = [0x83, 0x01, 0x82, 0x02, 0x03, 0xa1, 0x01, 0x61, 0x61];
        assert_eq!(canonical(&nested), Ok(()));
    }

    #[test]
    fn text_must_be_utf8() {
        assert_eq!(canonical(&[0x61, 0x61]), Ok(()), "\"a\"");
        assert_eq!(canonical(&[0x61, 0xff]), Err(Violation::NotUtf8));
    }

    #[test]
    fn one_item_and_no_more() {
        assert_eq!(canonical(&[]), Err(Violation::Truncated));
        assert_eq!(
            canonical(&[0x82, 0x01]),
            Err(Violation::Truncated),
            "an array short an item"
        );
        assert_eq!(canonical(&[0x00, 0x00]), Err(Violation::Trailing));
    }
}
