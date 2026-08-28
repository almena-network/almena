//! Writing and reading the canonical CBOR profile everything in this format is written in.
//!
//! `almena-cbor` says whether some bytes are canonical and deliberately does not encode — *"a
//! validator that only ever saw its own encoder's output would be a validator that agrees with a
//! bug."* This is the other half, kept in a different crate for exactly that reason, and every
//! test here hands what it wrote to that validator rather than to itself.
//!
//! **Map keys are unsigned integers and nothing else.** The profile calls for keys sorted by their
//! encoded bytes; for unsigned integers written in shortest form that order and plain numeric
//! order are the same, because a shorter encoding always begins with a smaller head. So a
//! [`BTreeMap<u64, _>`] is already in canonical order and there is no sort step to get wrong.
//!
//! Names, meanwhile, cost bytes in every copy of every operation forever, and the log entry is the
//! universal part and therefore the expensive one.

use std::collections::BTreeMap;

/// A value of the profile: what can appear in an operation, and nothing more.
///
/// There is no float variant and no negative variant, because the profile has neither. That is
/// not an omission to fill in later — a type that cannot represent a float is a stronger promise
/// than an encoder that declines to write one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    /// A non-negative integer.
    Uint(u64),
    /// A string of bytes — a hash, a key, a signature.
    Bytes(Vec<u8>),
    /// Text, which is UTF-8 by the time it is a `String`.
    Text(String),
    /// An ordered list.
    Array(Vec<Value>),
    /// A map with unsigned integer keys, already in canonical order by construction.
    Map(BTreeMap<u64, Value>),
    /// Absence, where the schema says a field is present and empty — `prev` on a first operation.
    Null,
}

impl Value {
    /// The canonical bytes of this value.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        self.write(&mut out);
        out
    }

    fn write(&self, out: &mut Vec<u8>) {
        match self {
            Self::Uint(n) => head(out, 0, *n),
            Self::Bytes(bytes) => {
                head(out, 2, bytes.len() as u64);
                out.extend_from_slice(bytes);
            }
            Self::Text(text) => {
                head(out, 3, text.len() as u64);
                out.extend_from_slice(text.as_bytes());
            }
            Self::Array(items) => {
                head(out, 4, items.len() as u64);
                for item in items {
                    item.write(out);
                }
            }
            Self::Map(fields) => {
                head(out, 5, fields.len() as u64);
                for (key, value) in fields {
                    head(out, 0, *key);
                    value.write(out);
                }
            }
            Self::Null => out.push(0xf6),
        }
    }
}

/// A head: the major type, and the argument in the shortest form that holds it.
fn head(out: &mut Vec<u8>, major: u8, argument: u64) {
    let major = major << 5;
    match argument {
        0..=23 => out.push(major | argument as u8),
        24..=0xff => out.extend_from_slice(&[major | 24, argument as u8]),
        0x100..=0xffff => {
            out.push(major | 25);
            out.extend_from_slice(&(argument as u16).to_be_bytes());
        }
        0x1_0000..=0xffff_ffff => {
            out.push(major | 26);
            out.extend_from_slice(&(argument as u32).to_be_bytes());
        }
        _ => {
            out.push(major | 27);
            out.extend_from_slice(&argument.to_be_bytes());
        }
    }
}

/// Why some bytes could not be read as a value of the profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unreadable {
    /// The bytes are not canonical. Which rule they broke is [`almena_cbor::Violation`]'s answer.
    NotCanonical(almena_cbor::Violation),
    /// Canonical CBOR, but carrying something this profile has no place for — a map key that is
    /// not an unsigned integer, or a simple value other than `null`.
    NotThisProfile,
}

/// Read canonical bytes back into a value.
///
/// **Canonicity is checked first, by the other crate**, and only then is anything decoded. That
/// order is the whole point: non-canonical bytes are invalid rather than something to normalise,
/// so bytes that fail are refused here and never re-encoded — re-encoding is what would break the
/// signature over them.
///
/// # Errors
///
/// [`Unreadable::NotCanonical`] when the bytes break the profile, [`Unreadable::NotThisProfile`]
/// when they are canonical CBOR that this schema still has no room for.
pub fn read(bytes: &[u8]) -> Result<Value, Unreadable> {
    almena_cbor::canonical(bytes).map_err(Unreadable::NotCanonical)?;
    let (value, rest) = value(bytes)?;
    if rest.is_empty() {
        Ok(value)
    } else {
        Err(Unreadable::NotThisProfile)
    }
}

/// One value off the front, and whatever follows it.
fn value(bytes: &[u8]) -> Result<(Value, &[u8]), Unreadable> {
    let (major, argument, rest) = split_head(bytes)?;
    match major {
        0 => Ok((Value::Uint(argument), rest)),
        2 => take(rest, argument).map(|(taken, rest)| (Value::Bytes(taken.to_vec()), rest)),
        3 => {
            let (taken, rest) = take(rest, argument)?;
            let text = core::str::from_utf8(taken).map_err(|_| Unreadable::NotThisProfile)?;
            Ok((Value::Text(text.to_owned()), rest))
        }
        4 => array(rest, argument),
        5 => map(rest, argument),
        7 if argument == 22 => Ok((Value::Null, rest)),
        _ => Err(Unreadable::NotThisProfile),
    }
}

/// The major type and the argument of the head at the front, and the bytes after it.
fn split_head(bytes: &[u8]) -> Result<(u8, u64, &[u8]), Unreadable> {
    let (&first, rest) = bytes.split_first().ok_or(Unreadable::NotThisProfile)?;
    let major = first >> 5;
    let short = first & 0x1f;
    let width = match short {
        0..=23 => return Ok((major, u64::from(short), rest)),
        24 => 1,
        25 => 2,
        26 => 4,
        27 => 8,
        _ => return Err(Unreadable::NotThisProfile),
    };
    let (taken, rest) = take(rest, width)?;
    let argument = taken
        .iter()
        .fold(0u64, |acc, &byte| (acc << 8) | u64::from(byte));
    Ok((major, argument, rest))
}

/// `count` bytes off the front, and whatever follows them.
fn take(bytes: &[u8], count: u64) -> Result<(&[u8], &[u8]), Unreadable> {
    let count = usize::try_from(count).map_err(|_| Unreadable::NotThisProfile)?;
    if bytes.len() < count {
        return Err(Unreadable::NotThisProfile);
    }
    Ok(bytes.split_at(count))
}

/// `count` values off the front, as an array.
fn array(mut bytes: &[u8], count: u64) -> Result<(Value, &[u8]), Unreadable> {
    let mut items = Vec::new();
    for _ in 0..count {
        let (item, rest) = value(bytes)?;
        items.push(item);
        bytes = rest;
    }
    Ok((Value::Array(items), bytes))
}

/// `count` pairs off the front, as a map. A key that is not an unsigned integer is refused.
fn map(mut bytes: &[u8], count: u64) -> Result<(Value, &[u8]), Unreadable> {
    let mut fields = BTreeMap::new();
    for _ in 0..count {
        let (major, key, rest) = split_head(bytes)?;
        if major != 0 {
            return Err(Unreadable::NotThisProfile);
        }
        let (value, rest) = value(rest)?;
        fields.insert(key, value);
        bytes = rest;
    }
    Ok((Value::Map(fields), bytes))
}

#[cfg(test)]
mod tests {
    use super::{Unreadable, Value, read};
    use std::collections::BTreeMap;

    /// Everything written here is handed to the other crate's validator, never to our own reader
    /// alone: an encoder checked against its own decoder agrees with its own bugs.
    fn round_trip(value: &Value) {
        let bytes = value.to_bytes();
        assert_eq!(
            almena_cbor::canonical(&bytes),
            Ok(()),
            "{value:?} was not canonical"
        );
        assert_eq!(read(&bytes), Ok(value.clone()));
    }

    fn map(pairs: &[(u64, Value)]) -> Value {
        Value::Map(pairs.iter().cloned().collect::<BTreeMap<_, _>>())
    }

    #[test]
    fn the_heads_are_written_in_their_shortest_form() {
        assert_eq!(Value::Uint(0).to_bytes(), [0x00]);
        assert_eq!(Value::Uint(23).to_bytes(), [0x17]);
        assert_eq!(Value::Uint(24).to_bytes(), [0x18, 0x18]);
        assert_eq!(Value::Uint(255).to_bytes(), [0x18, 0xff]);
        assert_eq!(Value::Uint(256).to_bytes(), [0x19, 0x01, 0x00]);
        assert_eq!(
            Value::Uint(65_536).to_bytes(),
            [0x1a, 0x00, 0x01, 0x00, 0x00]
        );
        assert_eq!(
            Value::Uint(u64::from(u32::MAX) + 1).to_bytes(),
            [0x1b, 0, 0, 0, 1, 0, 0, 0, 0]
        );
    }

    #[test]
    fn every_shape_of_the_profile_survives_a_round_trip() {
        for value in [
            Value::Uint(0),
            Value::Uint(u64::MAX),
            Value::Null,
            Value::Bytes(vec![]),
            Value::Bytes(vec![0xde, 0xad]),
            Value::Text(String::new()),
            Value::Text("una época".to_owned()),
            Value::Array(vec![]),
            Value::Array(vec![Value::Uint(1), Value::Null]),
            map(&[]),
            map(&[(1, Value::Uint(7)), (2, Value::Text("x".to_owned()))]),
        ] {
            round_trip(&value);
        }
    }

    #[test]
    fn a_map_comes_out_in_key_order_whatever_order_it_went_in() {
        // The property that lets there be no sort step: the key type is already ordered, and for
        // unsigned integers in shortest form that order is the profile's order.
        let mut fields = BTreeMap::new();
        for key in [300u64, 1, 24, 0, 23] {
            fields.insert(key, Value::Uint(key));
        }
        let bytes = Value::Map(fields).to_bytes();
        assert_eq!(almena_cbor::canonical(&bytes), Ok(()));
        assert_eq!(bytes[..3], [0xa5, 0x00, 0x00], "the map head, then key 0");
    }

    #[test]
    fn nesting_survives() {
        round_trip(&map(&[(
            1,
            Value::Array(vec![map(&[(9, Value::Bytes(vec![1, 2, 3]))]), Value::Null]),
        )]));
    }

    #[test]
    fn bytes_that_are_not_canonical_are_refused_rather_than_normalised() {
        // 24 written in two bytes when one would do. A tempting thing to accept and re-encode;
        // re-encoding is what would break the signature over it.
        let long_way = [0x18, 0x17];
        assert_eq!(
            read(&long_way),
            Err(Unreadable::NotCanonical(
                almena_cbor::Violation::NotShortest
            ))
        );
    }

    #[test]
    fn a_second_value_after_the_first_is_refused() {
        let two = [0x01, 0x02];
        assert!(read(&two).is_err());
    }

    #[test]
    fn a_map_key_that_is_not_a_number_is_refused() {
        // Canonical CBOR, but not this profile: {"a": 1}.
        let text_key = [0xa1, 0x61, 0x61, 0x01];
        assert_eq!(almena_cbor::canonical(&text_key), Ok(()));
        assert_eq!(read(&text_key), Err(Unreadable::NotThisProfile));
    }
}
