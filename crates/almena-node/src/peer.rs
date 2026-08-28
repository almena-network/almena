//! What a node is called on the mesh, worked out from the key it already has.
//!
//! A node has one key. It is what its roots are signed with, what names it in the record, and what
//! it will answer to when nodes talk to each other — **one key, three uses, and no second census**
//! to keep in step with the first.
//!
//! This is the third of those, written out. The mesh is not built, so nothing here connects to
//! anything; the name exists all the same, because it is a function of a key that is already on
//! disk and does not move. That is what makes it publishable before there is a mesh to use it —
//! and publishing it is the point, since a record saying *where* to call without saying *who*
//! answers would let a redirected address be a wrong node instead of a failed connection.
//!
//! # How the name is made
//!
//! Not our design; it is what everything else on that mesh already expects, and a name only one
//! implementation could work out would be no name at all.
//!
//! | | |
//! | --- | --- |
//! | 1 | The key goes in a two-field message: *which kind of key* and *the key* |
//! | 2 | That is short enough to need no hashing, so it is wrapped as itself rather than digested |
//! | 3 | The result is written in base58, the same alphabet everything else here is named in |
//!
//! Step 2 is the one worth saying out loud: the wrapper says *which* hash was used, and one of the
//! things it can say is **none**. A key of this size is carried whole, so the name contains the key
//! rather than a digest of it — which is why anybody holding the name can check a signature without
//! being told anything else.

use almena_format::identifier::base58;
use almena_suite::ed25519;

/// Which kind of key this is, in the field that says so.
///
/// Not a number this project chose. It is the one every other implementation on that mesh reads,
/// and a different one would produce a name nobody else arrives at.
const ED25519: u8 = 1;

/// The wrapper's way of saying **no hash was used**.
///
/// A key this short is carried whole rather than digested, so the name holds the key itself.
const NO_HASH: u8 = 0x00;

/// The field number carrying which kind of key it is, with its wire type.
const KIND_FIELD: u8 = 0x08;

/// The field number carrying the key, with its wire type.
const KEY_FIELD: u8 = 0x12;

/// What a node is called on the mesh.
///
/// A pure function of the key, so the same directory gives the same name every time it starts —
/// which is what makes it worth publishing where somebody else will read it.
#[must_use]
pub fn of(key: &ed25519::VerifyingKey) -> String {
    let public = key.bytes();

    let mut message = Vec::with_capacity(4 + public.len());
    message.push(KIND_FIELD);
    message.push(ED25519);
    message.push(KEY_FIELD);
    message.push(public.len() as u8);
    message.extend_from_slice(&public);

    let mut wrapped = Vec::with_capacity(2 + message.len());
    wrapped.push(NO_HASH);
    wrapped.push(message.len() as u8);
    wrapped.extend_from_slice(&message);

    base58(&wrapped)
}

#[cfg(test)]
mod tests {
    use super::of;
    use almena_suite::ed25519;

    #[test]
    fn it_matches_a_name_the_rest_of_the_world_produced() {
        // The only check that could catch a wrong field number, a wrong wrapper or a wrong
        // alphabet: a name made somewhere else entirely, taken apart, and arrived at again from
        // the key that was inside it. Two implementations of ours agreeing would prove nothing —
        // they would agree on the same mistake.
        const THEIRS: &str = "12D3KooWD3eckifWpRn9wQpMG9R9hX3sD158z7EqHWmweQAJU5SA";
        const KEY: [u8; 32] = [
            0x2f, 0xfa, 0x35, 0xa9, 0x9d, 0x3a, 0x3c, 0xfb, 0xb1, 0x7b, 0xb7, 0xc1, 0xdc, 0x55,
            0x61, 0xb1, 0x8a, 0x8d, 0xcc, 0xa4, 0xdf, 0x38, 0xdc, 0x61, 0x3e, 0xa8, 0x59, 0xc3,
            0x7e, 0xb1, 0x33, 0x6b,
        ];

        assert_eq!(
            of(&ed25519::VerifyingKey::from_bytes(KEY).expect("a key")),
            THEIRS
        );
    }

    #[test]
    fn every_name_this_makes_looks_like_one() {
        // The prefix is not decoration: it falls out of the wrapper, the field numbers and the key
        // length all being what they should be. A name that did not start this way would be one
        // nothing else on the mesh could read.
        for seed in [0u8, 1, 7, 200, 255] {
            let key = ed25519::SigningKey::from_secret([seed; 32]);
            let name = of(&key.verifying_key());
            assert!(name.starts_with("12D3KooW"), "{name}");
            assert_eq!(name.len(), 52, "{name}");
        }
    }

    #[test]
    fn one_key_is_one_name_however_often_it_is_asked() {
        let key = ed25519::SigningKey::from_secret([9; 32]);
        assert_eq!(of(&key.verifying_key()), of(&key.verifying_key()));
    }

    #[test]
    fn two_keys_are_two_names() {
        let one = ed25519::SigningKey::from_secret([1; 32]);
        let other = ed25519::SigningKey::from_secret([2; 32]);
        assert_ne!(of(&one.verifying_key()), of(&other.verifying_key()));
    }
}
