//! Asking a node, pinned to the node's own key.
//!
//! **Written by hand, the way the holder's app writes it.** The whole of what this needs from HTTP
//! is a request line, a couple of headers and a body, and the whole of what it needs from TLS is
//! that the key in the certificate is the key the node is named by. An HTTP client would bring a
//! trust store with it, and a trust store is a list of authorities this platform does not have:
//! nobody signs a node's certificate, the zone or a person says which key answers, and that key is
//! the only thing the connection is checked against.
//!
//! # What a node's identity looks like
//!
//! A libp2p peer identifier, base58btc: a multihash with code `0x00` (identity — the key is short
//! enough to carry whole) and length `0x24`, whose digest is a protobuf `PublicKey` with type
//! Ed25519 in field 1 and the thirty-two key bytes in field 2. So the bytes are exactly
//! `00 24 08 01 12 20` and the key, and anything of another shape is not a node's identity.
//!
//! # Where the key in the certificate is read from
//!
//! [`almena_tls::key_in`], which walks the certificate to its `SubjectPublicKeyInfo` and looks
//! nowhere else. A scan for the key's prefix anywhere in the bytes would be a way in: an impostor
//! could carry the pinned key in a name or an extension and present its own in the real place.

use std::sync::Arc;
use std::time::Duration;

use rustls_pki_types::{CertificateDer, ServerName, UnixTime};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;
use tokio_rustls::rustls::client::danger::{
    HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier,
};
use tokio_rustls::rustls::crypto::CryptoProvider;
use tokio_rustls::rustls::{ClientConfig, DigitallySignedStruct, Error, SignatureScheme};

use crate::answer::Answer;
use crate::failed::Failed;

/// How long a node has to answer before this stops waiting.
const PATIENCE: Duration = Duration::from_secs(10);

/// How long to wait for the connection itself.
const DIAL_PATIENCE: Duration = Duration::from_secs(3);

/// The largest answer this will read.
///
/// **A bound, because the other end is not this program.** A node paging a chain sends at most a
/// few megabytes; anything past this is not an answer.
const LARGEST: usize = 8 * 1024 * 1024;

/// What the bytes of a peer identifier begin with, before the key.
const PEER_HEAD: [u8; 6] = [0x00, 0x24, 0x08, 0x01, 0x12, 0x20];

/// How long the text of a peer identifier can be: fifty-two characters, rounded up.
const LONGEST_PEER: usize = 64;

/// Where a node is, and which key it has to answer with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    /// Host and port, resolved when dialled and not before.
    pub address: String,
    /// The node's own identifier: its peer identifier, which carries its public key.
    pub peer: String,
}

impl Node {
    /// A node at an origin — `https://host:port`, or the bare `host:port` — under that identity.
    ///
    /// # Errors
    ///
    /// `node_not_an_origin` for text that is neither, and the peer's own refusals.
    pub fn at(origin: &str, peer: &str) -> Result<Self, Failed> {
        let address = address_of(origin)?;
        key_of_peer(peer)?;
        Ok(Self {
            address,
            peer: peer.trim().to_owned(),
        })
    }

    /// The origin a peer identifier for this node would be written beside: `https://address`.
    #[must_use]
    pub fn origin(&self) -> String {
        format!("https://{}", self.address)
    }
}

/// The `host:port` an origin names.
///
/// `https` only, because a node serves under its own key and nothing else; text with no scheme is
/// taken as the address itself, which is how the node's own `--serve` flag writes it.
///
/// # Errors
///
/// `node_not_an_origin`.
pub fn address_of(origin: &str) -> Result<String, Failed> {
    let origin = origin.trim();
    if origin.starts_with("http://") {
        return Err(Failed::new("node_not_an_origin"));
    }
    let rest = origin.strip_prefix("https://").unwrap_or(origin);
    let authority = rest.split('/').next().unwrap_or_default();
    if authority.is_empty() || authority.contains(['@', ' ', '\r', '\n']) {
        return Err(Failed::new("node_not_an_origin"));
    }
    // A port is required: an origin without one would be dialled on 443, which is a number this
    // platform never chose, and a development node listens on whatever it was told.
    let after = authority.rsplit(':').next().unwrap_or_default();
    if after.is_empty() || after.parse::<u16>().is_err() || authority.ends_with(']') {
        return Err(Failed::new("node_not_an_origin"));
    }
    Ok(authority.to_owned())
}

/// The ed25519 public key a peer identifier names.
///
/// **Read and never guessed at.** A name of any other shape is refused rather than searched
/// through for thirty-two bytes that might be a key.
///
/// # Errors
///
/// `node_peer_not_base58` when the text is not base58, and `node_peer_not_an_identity` when the
/// bytes are not the one shape a node's identity has.
pub fn key_of_peer(peer: &str) -> Result<[u8; 32], Failed> {
    let peer = peer.trim();
    if peer.is_empty() || peer.len() > LONGEST_PEER {
        return Err(Failed::new("node_peer_not_an_identity"));
    }
    let decoded = almena_format::identifier::unbase58(peer)
        .ok_or_else(|| Failed::new("node_peer_not_base58"))?;
    decoded
        .strip_prefix(&PEER_HEAD[..])
        .and_then(|key| key.try_into().ok())
        .ok_or_else(|| Failed::new("node_peer_not_an_identity"))
}

/// The name a node writes for a key, which is what `key_of_peer` reads back.
#[must_use]
pub fn peer_of(key: &[u8; 32]) -> String {
    let mut bytes = PEER_HEAD.to_vec();
    bytes.extend_from_slice(key);
    almena_format::identifier::base58(&bytes)
}

/// Ask a node something at a path.
///
/// # Errors
///
/// A `node_*` word for the ways there was no answer.
pub async fn get(node: &Node, path: &str) -> Result<Answer, Failed> {
    asking(node, "GET", path, &[]).await
}

/// Hand a node something at a path: an act, a list, or post.
///
/// # Errors
///
/// A `node_*` word for the ways there was no answer. **A refusal is not one of them**: that comes
/// back as an answer with the rule in it.
pub async fn post(node: &Node, path: &str, body: &[u8]) -> Result<Answer, Failed> {
    asking(node, "POST", path, body).await
}

async fn asking(node: &Node, method: &str, path: &str, body: &[u8]) -> Result<Answer, Failed> {
    let request = written(method, path, &node.address, body);
    let bytes = exchange(node, &request).await?;
    Answer::read(body_of(&bytes)?)
}

/// The request, as bytes on the wire. `Connection: close`, and the answer is read to the end.
fn written(method: &str, path: &str, host: &str, body: &[u8]) -> Vec<u8> {
    let mut out = format!(
        "{method} {path} HTTP/1.1\r\nHost: {host}\r\nAccept: application/cbor\r\nConnection: close\r\n"
    )
    .into_bytes();
    if !body.is_empty() {
        out.extend_from_slice(
            format!(
                "Content-Type: application/cbor\r\nContent-Length: {}\r\n",
                body.len()
            )
            .as_bytes(),
        );
    }
    out.extend_from_slice(b"\r\n");
    out.extend_from_slice(body);
    out
}

/// What comes after the headers, which is the answer itself.
///
/// The status line is read and a body is taken only where the length says so. Every state a node
/// has an answer for is carried in the body, so `404` and `400` still hold an answer this reads.
fn body_of(response: &[u8]) -> Result<&[u8], Failed> {
    let at = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or_else(|| Failed::new("node_not_a_node"))?;
    let (head, body) = (&response[..at], &response[at + 4..]);
    let head = core::str::from_utf8(head).map_err(|_| Failed::new("node_not_a_node"))?;
    let mut lines = head.split("\r\n");
    let status = lines
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse::<u16>().ok())
        .ok_or_else(|| Failed::new("node_not_a_node"))?;
    if !matches!(status, 200 | 400 | 404 | 429) {
        return Err(Failed::with(
            "node_not_a_node",
            "status",
            &status.to_string(),
        ));
    }
    let mut length = None;
    for line in lines {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        let name = name.trim().to_ascii_lowercase();
        if name == "transfer-encoding" {
            return Err(Failed::new("node_not_a_node"));
        }
        if name == "content-length" {
            length = value.trim().parse::<usize>().ok();
        }
    }
    match length {
        Some(length) => body
            .get(..length)
            .ok_or_else(|| Failed::new("node_answer_cut_short")),
        None => Err(Failed::new("node_not_a_node")),
    }
}

/// The exchange itself: connect, verify against the node's own key, ask, read.
async fn exchange(node: &Node, request: &[u8]) -> Result<Vec<u8>, Failed> {
    let provider = Arc::new(tokio_rustls::rustls::crypto::ring::default_provider());
    let pinned = Pinned {
        expected: key_of_peer(&node.peer)?,
        provider: Arc::clone(&provider),
    };
    let settings = ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|_| Failed::new("node_unreachable"))?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(pinned))
        .with_no_client_auth();
    // Required by the protocol and meaning nothing here: the key is what is checked.
    let name =
        ServerName::try_from("node.almena.invalid").map_err(|_| Failed::new("node_unreachable"))?;

    let tcp = tokio::time::timeout(DIAL_PATIENCE, TcpStream::connect(node.address.as_str()))
        .await
        .map_err(|_| Failed::new("node_blocked"))?
        .map_err(|_| Failed::new("node_unreachable"))?;
    let mut stream = tokio::time::timeout(
        PATIENCE,
        TlsConnector::from(Arc::new(settings)).connect(name, tcp),
    )
    .await
    .map_err(|_| Failed::new("node_unreachable"))?
    .map_err(|error| {
        if error.to_string().contains("node_key_not_the_one_chosen") {
            Failed::new("node_not_that_node")
        } else {
            Failed::new("node_not_a_node")
        }
    })?;

    tokio::time::timeout(PATIENCE, async {
        stream.write_all(request).await?;
        stream.flush().await?;
        let mut answer = Vec::new();
        stream.take(LARGEST as u64).read_to_end(&mut answer).await?;
        Ok::<_, std::io::Error>(answer)
    })
    .await
    .map_err(|_| Failed::new("node_blocked"))?
    .map_err(|_| Failed::new("node_unreachable"))
}

/// A verifier that accepts one key and nothing else.
#[derive(Debug)]
struct Pinned {
    /// The key that was chosen, raw.
    expected: [u8; 32],
    /// What checks the handshake's own signatures, which is ordinary cryptography.
    provider: Arc<CryptoProvider>,
}

impl ServerCertVerifier for Pinned {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, Error> {
        // Not the chain, not the name, not the dates: the key is the whole question.
        match almena_tls::key_in(end_entity.as_ref()) {
            Ok(key) if key == self.expected => Ok(ServerCertVerified::assertion()),
            _ => Err(Error::General("node_key_not_the_one_chosen".to_owned())),
        }
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        // A node serves TLS 1.3 and nothing else.
        Err(Error::General("tls12_not_offered".to_owned()))
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        tokio_rustls::rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider
            .signature_verification_algorithms
            .supported_schemes()
    }
}

#[cfg(test)]
mod tests {
    use super::{Node, address_of, body_of, key_of_peer, peer_of, written};

    /// The pair the node repository itself is held to: a name libp2p produced, and its key.
    const THEIRS: &str = "12D3KooWD3eckifWpRn9wQpMG9R9hX3sD158z7EqHWmweQAJU5SA";
    const KEY: [u8; 32] = [
        0x2f, 0xfa, 0x35, 0xa9, 0x9d, 0x3a, 0x3c, 0xfb, 0xb1, 0x7b, 0xb7, 0xc1, 0xdc, 0x55, 0x61,
        0xb1, 0x8a, 0x8d, 0xcc, 0xa4, 0xdf, 0x38, 0xdc, 0x61, 0x3e, 0xa8, 0x59, 0xc3, 0x7e, 0xb1,
        0x33, 0x6b,
    ];

    #[test]
    fn a_name_the_rest_of_the_world_produced_yields_the_key_inside_it() {
        assert_eq!(key_of_peer(THEIRS), Ok(KEY));
        assert_eq!(peer_of(&KEY), THEIRS);
        assert_eq!(key_of_peer(&format!("  {THEIRS}\n")), Ok(KEY));
    }

    #[test]
    fn an_identifier_that_is_not_a_node_s_is_refused() {
        assert_eq!(
            key_of_peer("z6MkrgZRQmFYV67ovEyQwkvr6HnzJmFB2oFAVvuKnSpsU5s2")
                .unwrap_err()
                .to_string(),
            "node_peer_not_an_identity"
        );
        assert_eq!(
            key_of_peer("0O").unwrap_err().to_string(),
            "node_peer_not_base58"
        );
        assert!(key_of_peer("").is_err());
    }

    #[test]
    fn an_origin_is_https_with_a_port_and_the_address_is_what_is_dialled() {
        assert_eq!(
            address_of("https://127.0.0.1:8790").as_deref(),
            Ok("127.0.0.1:8790")
        );
        assert_eq!(address_of("[::1]:8790").as_deref(), Ok("[::1]:8790"));
        assert_eq!(
            address_of("https://madrid.dev.almena.network:8790/").as_deref(),
            Ok("madrid.dev.almena.network:8790")
        );
        for wrong in ["http://127.0.0.1:8790", "https://127.0.0.1", "", "https://"] {
            assert!(address_of(wrong).is_err(), "{wrong}");
        }
        let node = Node::at("https://127.0.0.1:8790", THEIRS).expect("a node");
        assert_eq!(node.origin(), "https://127.0.0.1:8790");
    }

    #[test]
    fn a_read_carries_no_caller_and_a_write_carries_only_the_body() {
        let read = String::from_utf8(written("GET", "/object/a", "here:8443", &[])).expect("ascii");
        assert!(read.starts_with("GET /object/a HTTP/1.1\r\n"));
        assert!(!read.to_lowercase().contains("authorization"));
        assert!(!read.contains("Content-Length"));
        let write = written("POST", "/acts", "here:8443", &[1, 2, 3]);
        assert!(write.ends_with(&[1, 2, 3]));
    }

    #[test]
    fn the_answer_is_what_the_length_says_and_a_short_one_is_no_answer() {
        assert_eq!(
            body_of(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n\x01\x02tail"),
            Ok(&[1u8, 2][..])
        );
        assert_eq!(
            body_of(b"HTTP/1.1 404 Not Found\r\nContent-Length: 1\r\n\r\n\x01"),
            Ok(&[1u8][..]),
            "a path a node does not serve still carries an answer"
        );
        assert_eq!(
            body_of(b"HTTP/1.1 200 OK\r\nContent-Length: 8\r\n\r\n\x01")
                .unwrap_err()
                .to_string(),
            "node_answer_cut_short"
        );
        assert!(body_of(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n").is_err());
        assert!(body_of(b"HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\n\r\n").is_err());
    }
}
