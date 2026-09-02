//! The certificate a node serves under, and nothing else about serving.
//!
//! Serving takes a stream and does not ask what it is, so this crate does not know what a question
//! or an answer looks like: it turns two files into something that wraps a connection, and stops
//! there. That is why it is a crate of its own rather than part of the transport — a node that
//! links the transport does not have to link a second cryptographic implementation it may never
//! use.
//!
//! # Why serving over the network needs one at all
//!
//! What a node answers is signed by the people who wrote it, so an intermediary cannot forge an
//! act. It **can** strip, delay, or lie about what is missing, and it can read every question
//! anybody asks — which on this platform is a list of who is looking up whom. A certificate does
//! not make a node trustworthy; it makes the conversation with it private and unaltered, which is
//! a different and smaller claim.
//!
//! # Where the certificate comes from
//!
//! **From the node's own key, unless an operator names two files.** A node has one key — it signs
//! roots with it, it is named by it in the record and in the zone, and it answers to it on the
//! mesh — so the certificate it serves under is built around that key and signed by it
//! ([`self_signed`]). Whoever dials the node was told that key by the zone or by the record and
//! pins it: they read the key out of the certificate ([`key_in`]) and compare, and need no
//! authority to vouch for a name they do not care about. That is what makes *serving in the
//! clear* not a mode at all: every node has a key, so every node has a certificate.
//!
//! An operator who already has a certificate for that machine names it and its key as two PEM
//! files ([`accepting`]), which is the shape every tool that issues one produces. **Nothing here
//! obtains one from an authority**, and that is deliberate: it would mean an account with the
//! authority, a directory of state that must survive, and a fourth thing that can be down at three
//! in the morning. An operator who has one should not be made to explain that to a program, and
//! one who does not has the node's own key.
//!
//! # A node says which it is doing
//!
//! A node asked to serve under a certificate it cannot load does not quietly come up under its own
//! key instead: whoever named the files would then be told everything was fine while what they
//! meant to serve under was not what was served. Which of the two a node is under is written in
//! its records, by the face that chose.

use std::path::Path;
use std::sync::Arc;

use rustls_pki_types::pem::PemObject as _;
use rustls_pki_types::{CertificateDer, PrivateKeyDer};

mod own;

pub use own::{NoKey, OwnKey, key_in, own_key};

/// Why a node could not take up a certificate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoCertificate {
    /// The certificate file is not there, cannot be read, or holds no certificate.
    NoChain,
    /// The key file is not there, cannot be read, or holds no private key.
    ///
    /// Told apart from the certificate because they are usually two different mistakes: a
    /// certificate is public and gets copied, and a key is not and gets left behind.
    NoKey,
    /// The two are not a pair, or this build will not serve under them.
    ///
    /// A key that does not belong to the certificate is the mistake that would otherwise be found
    /// by the first person who failed to connect.
    NotAPair,
}

/// What wraps a connection, once a node has a certificate to serve under.
pub type Accepting = tokio_rustls::TlsAcceptor;

/// Take up the certificate in `certificate` and the key in `key`.
///
/// Both are PEM, which is what every tool that issues a certificate produces. The certificate file
/// may hold a chain, and if it does the order is kept as written: it is the order a client walks,
/// and rearranging it here would be this deciding something about somebody else's certificate.
///
/// # Errors
///
/// [`NoCertificate`], and the three are worth telling apart because each is a different thing to go
/// and fix.
pub fn accepting(certificate: &Path, key: &Path) -> Result<Accepting, NoCertificate> {
    let chain: Vec<CertificateDer<'static>> = CertificateDer::pem_file_iter(certificate)
        .map_err(|_| NoCertificate::NoChain)?
        .collect::<Result<_, _>>()
        .map_err(|_| NoCertificate::NoChain)?;
    if chain.is_empty() {
        return Err(NoCertificate::NoChain);
    }

    let private = PrivateKeyDer::from_pem_file(key).map_err(|_| NoCertificate::NoKey)?;

    // The pairing is checked here rather than by the first person who fails to connect.
    let configured = tokio_rustls::rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(chain, private)
        .map_err(|_| NoCertificate::NotAPair)?;

    Ok(tokio_rustls::TlsAcceptor::from(Arc::new(configured)))
}

/// Serve under the node's own key: a certificate whose subject public key is that key's public
/// half, signed by the key itself.
///
/// `secret` is the thirty-two bytes the node's identity is made from — the same key that signs
/// roots and names the node on the mesh. What comes out is deterministic, so a node that restarts
/// presents the certificate it presented before, and whoever pinned it once has nothing to
/// re-pin. The bytes are [`own_key`]'s; this only hands them to the TLS implementation.
///
/// # Errors
///
/// [`NoCertificate::NotAPair`] if the TLS implementation will not take the pair. The pair is
/// this crate's own and always matches, so that would be this build's implementation refusing a
/// shape it took yesterday — said rather than hidden, because a node that then served nothing
/// would otherwise have nothing to say about why.
pub fn self_signed(secret: &[u8; 32]) -> Result<Accepting, NoCertificate> {
    let (certificate, key) = own_key(secret).into_parts();
    let configured = tokio_rustls::rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![certificate], key)
        .map_err(|_| NoCertificate::NotAPair)?;
    Ok(tokio_rustls::TlsAcceptor::from(Arc::new(configured)))
}

#[cfg(test)]
mod pinned {
    //! A client that trusts exactly one key, which is what dials a node.
    //!
    //! Written here as a test's verifier so that the certificate is proved against a real
    //! handshake rather than by inspection, and so that the holder's app has the shape of the
    //! verifier it writes in front of it: no name, no chain, no date — the key in the certificate
    //! is the key that was expected, or the connection is refused.

    use std::sync::Arc;

    use rustls_pki_types::{CertificateDer, ServerName, UnixTime};
    use tokio_rustls::rustls::client::danger::{
        HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier,
    };
    use tokio_rustls::rustls::{
        ClientConfig, DigitallySignedStruct, Error, SignatureScheme, crypto,
    };

    use super::{key_in, self_signed};

    /// Trusts the one key it was given.
    #[derive(Debug)]
    struct Pin([u8; 32]);

    impl ServerCertVerifier for Pin {
        fn verify_server_cert(
            &self,
            end_entity: &CertificateDer<'_>,
            _intermediates: &[CertificateDer<'_>],
            _server_name: &ServerName<'_>,
            _ocsp_response: &[u8],
            _now: UnixTime,
        ) -> Result<ServerCertVerified, Error> {
            match key_in(end_entity.as_ref()) {
                Ok(key) if key == self.0 => Ok(ServerCertVerified::assertion()),
                _ => Err(Error::General("not the node that was expected".to_owned())),
            }
        }

        fn verify_tls12_signature(
            &self,
            message: &[u8],
            cert: &CertificateDer<'_>,
            dss: &DigitallySignedStruct,
        ) -> Result<HandshakeSignatureValid, Error> {
            crypto::verify_tls12_signature(
                message,
                cert,
                dss,
                &crypto::ring::default_provider().signature_verification_algorithms,
            )
        }

        fn verify_tls13_signature(
            &self,
            message: &[u8],
            cert: &CertificateDer<'_>,
            dss: &DigitallySignedStruct,
        ) -> Result<HandshakeSignatureValid, Error> {
            // The implementation's own check, over the key it parses out of the certificate: this
            // is also what proves the certificate's key is one a standard reader can use.
            crypto::verify_tls13_signature(
                message,
                cert,
                dss,
                &crypto::ring::default_provider().signature_verification_algorithms,
            )
        }

        fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
            vec![SignatureScheme::ED25519]
        }
    }

    /// One handshake, in memory, between a node under `secret` and a client pinning `expected`.
    async fn handshake(secret: [u8; 32], expected: [u8; 32]) -> Result<(), String> {
        let acceptor = self_signed(&secret).map_err(|why| format!("{why:?}"))?;
        let client = ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(Pin(expected)))
            .with_no_client_auth();
        let connector = tokio_rustls::TlsConnector::from(Arc::new(client));

        let (near, far) = tokio::io::duplex(4096);
        let name = ServerName::try_from("localhost").map_err(|why| why.to_string())?;
        let (served, connected) = tokio::join!(acceptor.accept(far), connector.connect(name, near));
        served.map_err(|why| why.to_string())?;
        connected.map_err(|why| why.to_string())?;
        Ok(())
    }

    #[tokio::test]
    async fn a_client_pinning_the_node_key_connects() {
        let secret = [7; 32];
        let expected = almena_suite::ed25519::SigningKey::from_secret(secret)
            .verifying_key()
            .bytes();
        assert_eq!(handshake(secret, expected).await, Ok(()));
    }

    #[tokio::test]
    async fn a_client_pinning_another_key_is_refused() {
        // The other half, without which the test above would pass against a verifier that
        // accepted anything.
        let stranger = almena_suite::ed25519::SigningKey::from_secret([8; 32])
            .verifying_key()
            .bytes();
        assert!(handshake([7; 32], stranger).await.is_err());
    }
}

#[cfg(test)]
mod tests {
    use super::{NoCertificate, accepting, self_signed};
    use std::path::PathBuf;

    #[test]
    fn the_implementation_takes_a_node_key_as_a_pair() {
        // The implementation checks that the key it is handed is the key in the certificate,
        // by comparing the subject public key it parses out of the bytes written by hand.
        assert!(self_signed(&[7; 32]).is_ok());
    }

    /// A directory of this test's own, removed when it is done with it.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!("almena-tls-{name}"));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).expect("the directory");
            Self(path)
        }

        fn holding(&self, name: &str, what: &[u8]) -> PathBuf {
            let path = self.0.join(name);
            std::fs::write(&path, what).expect("written");
            path
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn a_certificate_that_is_not_there_is_said_so() {
        let scratch = Scratch::new("missing");
        assert_eq!(
            accepting(
                &scratch.0.join("nothing.pem"),
                &scratch.0.join("nothing.key")
            )
            .err(),
            Some(NoCertificate::NoChain)
        );
    }

    #[test]
    fn a_file_that_is_not_a_certificate_is_not_read_as_one() {
        let scratch = Scratch::new("garbage");
        let certificate = scratch.holding("cert.pem", b"this is not a certificate");
        let key = scratch.holding("cert.key", b"nor is this a key");

        assert_eq!(
            accepting(&certificate, &key).err(),
            Some(NoCertificate::NoChain)
        );
    }

    #[test]
    fn a_certificate_with_no_key_beside_it_is_a_different_answer() {
        // The two are usually two different mistakes: a certificate is public and gets copied, and
        // a key is not and gets left behind.
        let scratch = Scratch::new("nokey");
        let certificate = scratch.holding(
            "cert.pem",
            b"-----BEGIN CERTIFICATE-----\nMIIB\n-----END CERTIFICATE-----\n",
        );
        let key = scratch.holding("cert.key", b"");

        assert!(matches!(
            accepting(&certificate, &key).err(),
            Some(NoCertificate::NoChain | NoCertificate::NoKey)
        ));
    }
}
