//! AES key wrap, RFC 3394, written out over the block cipher.
//!
//! **Written rather than depended on**, because the holder's app takes it from a crate this
//! workspace does not hold, and what the two have to agree on is the number the RFC publishes and
//! not a crate. The algorithm is six rounds of the block cipher over the key in 64-bit halves,
//! with a running counter folded into the first half; the integrity check is a fixed value that
//! has to come out of the other end intact.
//!
//! Only the 256-bit key-encryption key, because that is the one `ECDH-1PU+A256KW` names.

use aes::Aes256;
use aes::cipher::generic_array::GenericArray;
use aes::cipher::{BlockDecrypt as _, BlockEncrypt as _, KeyInit as _};

/// The initial value the RFC fixes, which is what has to come back out of an unwrap.
const IV: [u8; 8] = [0xA6; 8];

/// How many times the whole key is passed over.
const ROUNDS: u32 = 6;

/// Why bytes could not be wrapped or unwrapped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotWrapped {
    /// A key to wrap has to be whole 64-bit blocks, at least two of them.
    Shape,
    /// The integrity check did not come out, so the key or the wrapping is not what it claims.
    Integrity,
}

/// Wrap `plain` under `kek`.
///
/// # Errors
///
/// [`NotWrapped::Shape`] for a key that is not two or more whole blocks of eight bytes.
pub fn wrap(kek: &[u8; 32], plain: &[u8]) -> Result<Vec<u8>, NotWrapped> {
    if plain.len() < 16 || !plain.len().is_multiple_of(8) {
        return Err(NotWrapped::Shape);
    }
    let cipher = Aes256::new(GenericArray::from_slice(kek));
    let n = plain.len() / 8;
    let mut a = IV;
    let mut r: Vec<[u8; 8]> = plain
        .chunks(8)
        .map(|chunk| {
            let mut out = [0u8; 8];
            out.copy_from_slice(chunk);
            out
        })
        .collect();
    for j in 0..ROUNDS {
        for (i, held) in r.iter_mut().enumerate() {
            let mut block = [0u8; 16];
            block[..8].copy_from_slice(&a);
            block[8..].copy_from_slice(held);
            cipher.encrypt_block(GenericArray::from_mut_slice(&mut block));
            let t = (n as u64) * u64::from(j) + (i as u64 + 1);
            a.copy_from_slice(&block[..8]);
            for (byte, counter) in a.iter_mut().zip(t.to_be_bytes()) {
                *byte ^= counter;
            }
            held.copy_from_slice(&block[8..]);
        }
    }
    let mut out = a.to_vec();
    for held in r {
        out.extend_from_slice(&held);
    }
    Ok(out)
}

/// Unwrap `wrapped` under `kek`.
///
/// # Errors
///
/// [`NotWrapped::Shape`] for bytes that are not three or more whole blocks, and
/// [`NotWrapped::Integrity`] when the fixed value does not come out — which is one error for a
/// wrong key and for altered bytes, because telling them apart tells an attacker which to fix.
pub fn unwrap(kek: &[u8; 32], wrapped: &[u8]) -> Result<Vec<u8>, NotWrapped> {
    if wrapped.len() < 24 || !wrapped.len().is_multiple_of(8) {
        return Err(NotWrapped::Shape);
    }
    let cipher = Aes256::new(GenericArray::from_slice(kek));
    let n = wrapped.len() / 8 - 1;
    let mut a = [0u8; 8];
    a.copy_from_slice(&wrapped[..8]);
    let mut r: Vec<[u8; 8]> = wrapped[8..]
        .chunks(8)
        .map(|chunk| {
            let mut out = [0u8; 8];
            out.copy_from_slice(chunk);
            out
        })
        .collect();
    for j in (0..ROUNDS).rev() {
        for i in (0..n).rev() {
            let t = (n as u64) * u64::from(j) + (i as u64 + 1);
            let mut block = [0u8; 16];
            block[..8].copy_from_slice(&a);
            for (byte, counter) in block[..8].iter_mut().zip(t.to_be_bytes()) {
                *byte ^= counter;
            }
            block[8..].copy_from_slice(&r[i]);
            cipher.decrypt_block(GenericArray::from_mut_slice(&mut block));
            a.copy_from_slice(&block[..8]);
            r[i].copy_from_slice(&block[8..]);
        }
    }
    if a != IV {
        return Err(NotWrapped::Integrity);
    }
    let mut out = Vec::with_capacity(n * 8);
    for held in r {
        out.extend_from_slice(&held);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::{NotWrapped, unwrap, wrap};
    use crate::directory::unhex;

    /// The 256-bit key-encryption key every vector in RFC 3394 §4 with a 256-bit KEK uses.
    const KEK: &str = "000102030405060708090A0B0C0D0E0F101112131415161718191A1B1C1D1E1F";

    fn kek() -> [u8; 32] {
        unhex(KEK).expect("hex").try_into().expect("32 bytes")
    }

    #[test]
    fn it_agrees_with_the_numbers_in_the_standard_for_a_256_bit_key() {
        // **RFC 3394 §4.6**: 256 bits of key data under a 256-bit KEK. A wrap that round-trips with
        // itself and nothing else can be confidently, symmetrically wrong.
        let key =
            unhex("00112233445566778899AABBCCDDEEFF000102030405060708090A0B0C0D0E0F").expect("hex");
        let expected = unhex(
            "28C9F404C4B810F4CBCCB35CFB87F8263F5786E2D80ED326CBC7F0E71A99F43BFB988B9B7A02DD21",
        )
        .expect("hex");
        assert_eq!(wrap(&kek(), &key), Ok(expected.clone()));
        assert_eq!(unwrap(&kek(), &expected), Ok(key));
    }

    #[test]
    fn it_agrees_with_the_numbers_in_the_standard_for_a_128_bit_key() {
        // **RFC 3394 §4.3**: 128 bits of key data under a 256-bit KEK.
        let key = unhex("00112233445566778899AABBCCDDEEFF").expect("hex");
        let expected = unhex("64E8C3F9CE0F5BA263E9777905818A2A93C8191E7D6E8AE7").expect("hex");
        assert_eq!(wrap(&kek(), &key), Ok(expected.clone()));
        assert_eq!(unwrap(&kek(), &expected), Ok(key));
    }

    #[test]
    fn a_wrong_key_and_altered_bytes_are_one_failure() {
        let wrapped = wrap(&kek(), &[7; 32]).expect("wrapped");
        let mut altered = wrapped.clone();
        altered[9] ^= 1;
        assert_eq!(unwrap(&kek(), &altered), Err(NotWrapped::Integrity));
        assert_eq!(unwrap(&[9; 32], &wrapped), Err(NotWrapped::Integrity));
    }

    #[test]
    fn a_key_that_is_not_whole_blocks_is_refused() {
        assert_eq!(wrap(&kek(), &[1; 20]), Err(NotWrapped::Shape));
        assert_eq!(wrap(&kek(), &[1; 8]), Err(NotWrapped::Shape));
        assert_eq!(unwrap(&kek(), &[1; 16]), Err(NotWrapped::Shape));
    }
}
