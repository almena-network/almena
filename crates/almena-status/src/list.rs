//! The bitstring itself: how it is written, how it is read, and where an index comes from.
//!
//! # The encoding is the W3C's and not this project's
//!
//! `SPECS.md §10.2` fixes **Bitstring Status List**: the bits, GZIP over them, and multibase
//! base64url over that. Reusing it rather than inventing one is the same rule the attribute core
//! follows, and it is what lets a verifier written against the standard read one of these.
//!
//! # The floor is what stops the index from being an identifier
//!
//! `SPECS.md §10.2` puts it at **131 072 entries**. An issuer with few credentials would otherwise
//! have a small list, and an index inside a small list identifies almost directly. Sixteen
//! kilobytes raw and almost nothing compressed, because a bitstring of nearly all zeros compresses
//! away.

use std::io::{Read as _, Write as _};

use almena_credential::base64url;
use almena_suite::digest::Digest;

/// The fewest entries any list has.
///
/// **The published recommendation of the specification** (`SPECS.md §10.2`), and the reason a small
/// issuer's index is not an identifier.
pub const AT_LEAST: u64 = 131_072;

/// What multibase calls base64url without padding, which is the prefix the encoded list carries.
const MULTIBASE: char = 'u';

/// One issuer's revocations for one cohort.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct List {
    /// One bit per entry, most significant bit of each byte first, as the specification writes them.
    bits: Vec<u8>,
}

/// The operating system would not produce randomness.
///
/// **Its own type rather than the generator's**, so that whoever calls this does not have to name a
/// dependency to handle it — and so that changing where randomness comes from is one edit here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoRandomness;

/// Why some bytes are not a list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotAList {
    /// The text does not begin with the one multibase prefix this reads.
    NotMultibase,
    /// What follows the prefix is not base64url.
    NotBase64Url,
    /// The bytes are not GZIP, or are GZIP of something this cannot use.
    NotCompressed,
    /// It holds fewer entries than every list holds.
    ///
    /// **Refused rather than padded on the way in.** A short list read as though it were padded
    /// would be a list whose bytes and whose hash disagree about what it says.
    TooSmall,
}

impl List {
    /// A list with nothing revoked in it, at the smallest size any list has.
    #[must_use]
    pub fn empty() -> Self {
        Self::of(AT_LEAST)
    }

    /// A list of that many entries, rounded up to the floor and to a whole byte.
    #[must_use]
    pub fn of(entries: u64) -> Self {
        let entries = entries.max(AT_LEAST);
        let width = usize::try_from(entries.div_ceil(8)).unwrap_or(usize::MAX);
        Self {
            bits: vec![0; width],
        }
    }

    /// How many entries it holds.
    #[must_use]
    pub fn entries(&self) -> u64 {
        self.bits.len() as u64 * 8
    }

    /// Whether that index is set.
    ///
    /// **An index past the end is not set**, which is the honest answer: this list says nothing
    /// about it. Whether a credential pointing past the end is one to accept is the verifier's
    /// question and not this one's.
    #[must_use]
    pub fn revoked(&self, index: u64) -> bool {
        let (byte, bit) = place(index);
        self.bits.get(byte).is_some_and(|held| held & bit != 0)
    }

    /// Set that index, which is what revoking is.
    ///
    /// **One direction only.** Un-revoking is not a thing this build writes: the format leaves room
    /// for suspension and `SPECS.md §10.1` deliberately does not use it, so a bit that went back to
    /// nought would be a state nobody specified arriving at a verifier that has no reading for it.
    pub fn revoke(&mut self, index: u64) {
        let (byte, bit) = place(index);
        if let Some(held) = self.bits.get_mut(byte) {
            *held |= bit;
        }
    }

    /// How many are revoked, which is what a page showing a list says.
    #[must_use]
    pub fn how_many(&self) -> u32 {
        self.bits.iter().map(|byte| byte.count_ones()).sum()
    }

    /// The list as it travels: GZIP, then multibase base64url.
    #[must_use]
    pub fn written(&self) -> String {
        let mut writer = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::best());
        // Writing to a `Vec` cannot fail, and the encoder's own finish only fails on the writer.
        let _ = writer.write_all(&self.bits);
        let compressed = writer.finish().unwrap_or_default();
        format!("{MULTIBASE}{}", base64url::encode(&compressed))
    }

    /// One read back from how it was written.
    ///
    /// # Errors
    ///
    /// [`NotAList`], telling apart each of the four ways some text is not one.
    pub fn read(written: &str) -> Result<Self, NotAList> {
        let held = written
            .strip_prefix(MULTIBASE)
            .ok_or(NotAList::NotMultibase)?;
        let compressed = base64url::decode(held).map_err(|_| NotAList::NotBase64Url)?;
        let mut bits = Vec::new();
        flate2::read::GzDecoder::new(compressed.as_slice())
            // Bounded, because the other end is not this program: a list that expands without limit
            // is one whoever served it chose the memory cost of.
            .take(AT_LEAST.saturating_mul(64))
            .read_to_end(&mut bits)
            .map_err(|_| NotAList::NotCompressed)?;
        if (bits.len() as u64) < AT_LEAST / 8 {
            return Err(NotAList::TooSmall);
        }
        Ok(Self { bits })
    }

    /// What the record holds about this version: the hash of the bytes that travel.
    ///
    /// **The hash decides, so any source will do** (`SPECS.md §10.2`). Whoever serves the bytes is
    /// indifferent: either they match the version the record names or they do not.
    #[must_use]
    pub fn version(&self) -> Digest {
        Digest::of(self.written().as_bytes())
    }
}

/// Where an index sits: which byte, and which bit of it.
///
/// **Most significant bit first**, which is how the specification numbers them. Getting this the
/// other way round would make one list say two things to two implementations.
const fn place(index: u64) -> (usize, u8) {
    ((index / 8) as usize, 1 << (7 - (index % 8)))
}

/// An index drawn at random inside a list.
///
/// **At random, never in sequence** (`SPECS.md §10.2`). A sequential index reveals how long
/// somebody has been a customer — an attribute the holder never agreed to disclose, travelling in
/// every presentation they make.
///
/// Collisions are the caller's to notice: with 131 072 entries and a handful of credentials, one is
/// unlikely, and an issuer that keeps the index↔holder correspondence anyway (`SPECS.md §10.2`) is
/// the one place that can say whether an index is taken.
///
/// # Errors
///
/// [`NoRandomness`] when the operating system will not produce any. Refused rather than worked
/// around: falling back to a counter would put back the exact fact the randomness removes — how
/// long somebody has been a customer, travelling in every presentation they make.
pub fn somewhere(entries: u64) -> Result<u64, NoRandomness> {
    let mut drawn = [0u8; 8];
    getrandom::fill(&mut drawn).map_err(|_| NoRandomness)?;
    Ok(u64::from_be_bytes(drawn) % entries.max(AT_LEAST))
}

#[cfg(test)]
mod tests {
    use super::{AT_LEAST, List, NotAList, somewhere};

    #[test]
    fn a_list_is_never_smaller_than_the_floor() {
        // **What stops an index being an identifier**: a small issuer would otherwise have a small
        // list, and a place in a small list names almost directly.
        assert_eq!(List::of(10).entries(), AT_LEAST);
        assert_eq!(List::empty().entries(), AT_LEAST);
        assert!(List::of(AT_LEAST * 4).entries() >= AT_LEAST * 4);
    }

    #[test]
    fn a_list_reads_back_as_itself_and_the_hash_is_over_what_travels() {
        let mut held = List::empty();
        held.revoke(0);
        held.revoke(4242);
        held.revoke(AT_LEAST - 1);

        let written = held.written();
        let read = List::read(&written).expect("a list");
        assert_eq!(read, held);
        assert_eq!(read.version(), held.version());
        assert!(read.revoked(0) && read.revoked(4242) && read.revoked(AT_LEAST - 1));
        assert!(!read.revoked(1));
        assert_eq!(read.how_many(), 3);
    }

    #[test]
    fn a_list_of_almost_all_noughts_compresses_to_almost_nothing() {
        // Which is what makes the floor free, and the sparse index with it.
        let written = List::empty().written();
        assert!(
            written.len() < 512,
            "sixteen kilobytes of noughts came to {} characters",
            written.len()
        );
    }

    #[test]
    fn something_that_is_not_a_list_says_which_way_it_is_not_one() {
        assert_eq!(List::read("zSomething"), Err(NotAList::NotMultibase));
        assert_eq!(List::read("u not base64url!"), Err(NotAList::NotBase64Url));
        assert_eq!(List::read("uAAAA"), Err(NotAList::NotCompressed));

        // A short list padded on the way in would be one whose bytes and whose hash disagree about
        // what it says, so it is refused instead.
        let short = List { bits: vec![0; 8] };
        assert_eq!(List::read(&short.written()), Err(NotAList::TooSmall));
    }

    #[test]
    fn an_index_past_the_end_is_a_list_saying_nothing_about_it() {
        // The honest answer. Whether a credential pointing past the end is one to accept is the
        // verifier's question, and answering it here would be answering it for everybody.
        let mut held = List::empty();
        held.revoke(AT_LEAST * 2);
        assert!(!held.revoked(AT_LEAST * 2));
        assert_eq!(held.how_many(), 0);
    }

    #[test]
    fn an_index_is_drawn_from_the_whole_space_and_not_from_the_front() {
        // A sequential index is an attribute the holder never disclosed, travelling in every
        // presentation.
        let drawn: Vec<u64> = (0..64)
            .map(|_| somewhere(AT_LEAST).expect("randomness"))
            .collect();
        assert!(drawn.iter().all(|one| *one < AT_LEAST));
        assert!(
            drawn.iter().any(|one| *one > AT_LEAST / 2),
            "and not all of them landed in the first half"
        );
        assert!(drawn.windows(2).any(|pair| pair[0] != pair[1]));
    }
}
