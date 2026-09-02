//! The links this program reads and writes, which are what a QR code would carry.
//!
//! `almena://<what>?<field>=<value>`, percent-encoded. Two kinds pass through here: the one a
//! holder shows to be met (`meet?who=`), which the partner reads, and the one a verifier shows to
//! be answered (`present?request=`), which the partner writes. The pointer inside the second is
//! the sign-in pointer's four fields, unchanged; what differs is the kind.

use crate::failed::Failed;

/// What a verifier's code carries, which is a pointer and nothing else.
///
/// Four fields, the same four a sign-in pointer has. A code carrying a fifth would be a code
/// somebody could draw a screen from, and the far end deciding what a person sees is what having
/// the wallet draw the screen exists to prevent.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Pointer {
    /// The verifier's identifier, which is what its standing is resolved against.
    pub verifier: String,
    /// Where the request is fetched from and the answer goes to.
    pub at: String,
    /// The challenge.
    pub nonce: String,
    /// The epoch it stops being good in, on the network's clock.
    pub until: u64,
}

/// The link a wallet reads to answer a request.
///
/// # Errors
///
/// `link_not_written`, which serialising four strings and a number cannot produce.
pub fn present(pointer: &Pointer) -> Result<String, Failed> {
    let json = serde_json::to_string(pointer).map_err(|_| Failed::new("link_not_written"))?;
    Ok(format!("almena://present?request={}", encoded(&json)))
}

/// The peer identifier a `meet` link carries, or the bare identifier where that is what was given.
///
/// # Errors
///
/// `link_not_a_meeting` for a link of another kind or one missing its field.
pub fn met(text: &str) -> Result<String, Failed> {
    let text = text.trim();
    if text.starts_with("did:peer:2.") {
        return Ok(text.to_owned());
    }
    let query = text
        .strip_prefix("almena://meet?")
        .ok_or_else(|| Failed::new("link_not_a_meeting"))?;
    query
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .find(|(field, _)| *field == "who")
        .and_then(|(_, value)| decoded(value))
        .filter(|who| !who.is_empty())
        .ok_or_else(|| Failed::new("link_not_a_meeting"))
}

/// Percent-encoding as a browser's `encodeURIComponent` does it: the unreserved characters stay,
/// everything else is `%XX` over its UTF-8 bytes.
#[must_use]
pub fn encoded(text: &str) -> String {
    let mut out = String::with_capacity(text.len() * 3);
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || b"-_.~".contains(&byte) {
            out.push(char::from(byte));
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// The same, read back; nothing for text that is not well-formed.
#[must_use]
pub fn decoded(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at] == b'%' {
            let pair = bytes.get(at + 1..at + 3)?;
            let hex = core::str::from_utf8(pair).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            at += 3;
        } else {
            out.push(if bytes[at] == b'+' { b' ' } else { bytes[at] });
            at += 1;
        }
    }
    String::from_utf8(out).ok()
}

#[cfg(test)]
mod tests {
    use super::{Pointer, decoded, encoded, met, present};

    #[test]
    fn a_pointer_travels_percent_encoded_and_comes_back_as_the_same_four_fields() {
        let pointer = Pointer {
            verifier: "did:almena:dev:zVerifier".to_owned(),
            at: "http://127.0.0.1:8899/present".to_owned(),
            nonce: "a nonce/with?odd&chars".to_owned(),
            until: 73,
        };
        let link = present(&pointer).expect("a link");
        assert!(
            link.starts_with("almena://present?request=%7B%22verifier%22"),
            "{link}"
        );
        let query = link
            .strip_prefix("almena://present?request=")
            .expect("the field");
        let read: Pointer = serde_json::from_str(&decoded(query).expect("decodes")).expect("json");
        assert_eq!(read, pointer);
    }

    #[test]
    fn a_meeting_link_carries_a_peer_identifier_and_a_bare_one_is_taken_as_it_is() {
        let who = "did:peer:2.Vzabc.Ezabc.SaHR0cHM6Ly9h";
        assert_eq!(
            met(&format!("almena://meet?who={}", encoded(who))).as_deref(),
            Ok(who)
        );
        assert_eq!(met(who).as_deref(), Ok(who));
        assert!(met("almena://node?address=a&peer=b").is_err());
        assert!(met("almena://meet?who=").is_err());
    }

    #[test]
    fn the_encoding_is_the_one_a_browser_reads_back() {
        assert_eq!(encoded("a b/c?d=e&f"), "a%20b%2Fc%3Fd%3De%26f");
        assert_eq!(decoded("a%20b+c"), Some("a b c".to_owned()));
        assert_eq!(decoded("%zz"), None);
        assert_eq!(decoded("%2"), None);
    }
}
