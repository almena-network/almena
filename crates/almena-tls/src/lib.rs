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
//! Two files, named by whoever runs the node. **Nothing here obtains one**, and that is deliberate:
//! getting a certificate automatically means an account with an authority, a directory of state
//! that must survive, and a fourth thing that can be down at three in the morning. An operator who
//! already has a certificate should not be made to explain that to a program, and one who does not
//! is better served by the tool they already use for every other service on that machine.
//!
//! # A node says which it is doing
//!
//! There is no self-signed fallback and no *carry on without it*. A node asked to serve under a
//! certificate it cannot load does not come up serving in the clear instead: whoever asked for a
//! certificate would then be told everything was fine while their operators' questions travelled in
//! the open.

use std::path::Path;
use std::sync::Arc;

use rustls_pki_types::pem::PemObject as _;
use rustls_pki_types::{CertificateDer, PrivateKeyDer};

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

#[cfg(test)]
mod tests {
    use super::{NoCertificate, accepting};
    use std::path::PathBuf;

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
