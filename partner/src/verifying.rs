//! `verify`: serve a request, take a presentation, and say one of three things about it.
//!
//! # The endpoint a wallet talks to
//!
//! One path, two methods. `GET {path}?nonce={nonce}` hands over the request object — a DCQL query
//! over OpenID4VP, exactly as `almena_sdk::request::written` writes it, which references the
//! template by hash and carries a purpose per attribute. `POST {path}` takes JSON
//! `{"nonce": …, "presentation": "…"}` or `{"nonce": …, "refused": true}` and answers
//! `200 {"outcome": "accepted" | "not_what_was_asked" | "could_not_verify", "why": …}`, `400` for
//! a body that is not one of those, and `410` for a nonce this run is not asking about or one
//! whose moment has passed.
//!
//! # The three answers stay three
//!
//! *Accepted* is a presentation that holds up and answers the request. *Not what was asked* is
//! everything about the presentation itself — a refusal, a missing attribute, a bad signature, a
//! revocation actually seen. *Could not verify* is about this verifier's reach — an issuer it could
//! not resolve, a status list it could not obtain or one older than the hash in sight — and it is
//! never dressed up as the other, because a verifier that conflated them would teach its staff to
//! wave people through when the network fails.
//!
//! # What it serves under
//!
//! Its own Ed25519 key's certificate, the same shape a node serves under, when asked; an
//! operator's PEM pair when given one; and plain HTTP otherwise, for a wallet built for
//! development on this same machine, which is the one case a wallet answers in the clear. Which of
//! the three is written in the records as `under=`.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use almena_credential::Method;
use almena_credential::verify::{Fault, Missing, Revocation};
use almena_format::identifier::{Did, Name};
use almena_sdk::request::{Request, Wanted, holds_up, written};
use almena_sdk::verifier::{Against, Policy, Resolved, Says, Unanswered, verify};
use almena_status::wanted::{Reached, what_is_known};
use almena_suite::p256;
use almena_time::Epoch;
use tokio::io::{AsyncRead, AsyncReadExt as _, AsyncWrite, AsyncWriteExt as _};
use tokio::net::TcpListener;

use crate::chain;
use crate::commands::{Partner, drawn_name};
use crate::failed::Failed;
use crate::link::{self, Pointer};

/// How long one wallet's connection may take, end to end.
const PATIENCE: Duration = Duration::from_secs(10);

/// The largest request this will read: a presentation is a few kilobytes.
const LARGEST: usize = 256 * 1024;

/// How long the nonce stays askable, on the network's clock: this epoch and the next.
const OPEN_FOR: u64 = 1;

/// What the endpoint serves under.
#[derive(Debug, Clone)]
pub enum Under {
    /// Plain HTTP, for a wallet built for development on this machine.
    Nothing,
    /// The partner's own key, as a self-signed certificate.
    OwnKey,
    /// An operator's PEM pair.
    Certificate {
        /// The certificate file.
        certificate: PathBuf,
        /// The key file.
        key: PathBuf,
    },
}

impl Under {
    /// The word the records carry for it.
    #[must_use]
    pub const fn word(&self) -> &'static str {
        match self {
            Self::Nothing => "nothing",
            Self::OwnKey => "own_key",
            Self::Certificate { .. } => "a_certificate",
        }
    }
}

/// What an operator asks to be verified.
#[derive(Debug, Clone)]
pub struct Asking {
    /// The verifier, which is who the presentation is for.
    pub verifier: Did,
    /// The request template version, by hash.
    pub template: Name,
    /// The credential shapes taken, by hash; empty is any.
    pub accepts: Vec<Name>,
    /// What is asked for: attribute and purpose.
    pub asks: Vec<(Name, String)>,
    /// Where to listen: `host:port`.
    pub serve: String,
    /// The path the wallet talks to.
    pub path: String,
    /// What to serve under.
    pub under: Under,
    /// Whether a credential that cannot be revoked is refused.
    pub require_revocable: bool,
}

/// The three answers, as words on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// It holds up and answers the request.
    Accepted,
    /// It does not, and that is about the presentation.
    NotWhatWasAsked,
    /// Nothing could be concluded, and that is about this verifier's reach.
    CouldNotVerify,
}

impl Outcome {
    /// The word.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::NotWhatWasAsked => "not_what_was_asked",
            Self::CouldNotVerify => "could_not_verify",
        }
    }
}

/// What one presentation came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Judged {
    /// Which of the three.
    pub outcome: Outcome,
    /// Why, where it was not accepted: an identifier, never a sentence.
    pub why: Option<String>,
}

/// The endpoint, up, until one presentation has been judged.
pub struct Started {
    /// Where it is listening.
    pub address: std::net::SocketAddr,
    /// The link a wallet reads.
    pub link: String,
    /// The pointer inside it.
    pub pointer: Pointer,
    /// The request object it serves.
    pub request: serde_json::Value,
    judged: tokio::sync::oneshot::Receiver<Judged>,
}

impl Started {
    /// Wait for the one presentation, and what it came to.
    ///
    /// # Errors
    ///
    /// `verify_stopped` when the endpoint went away before anybody presented.
    pub async fn judged(self) -> Result<Judged, Failed> {
        self.judged.await.map_err(|_| Failed::new("verify_stopped"))
    }
}

/// Compose the request, bind the endpoint, and start answering.
///
/// # Errors
///
/// `verify_request_malformed` naming what put the request over its template, `verify_not_bound`,
/// `verify_no_certificate`, and what the node fails with.
pub async fn start(partner: &Partner, asking: &Asking) -> Result<Started, Failed> {
    let (_, version) = chain::template_version(&partner.node, &asking.template).await?;
    let network = chain::network(&partner.node).await?;
    let request = composed(asking, &version)?;
    holds_up(&request, &version)
        .map_err(|why| Failed::with("verify_request_malformed", "why", &format!("{why:?}")))?;

    let listener = TcpListener::bind(&asking.serve)
        .await
        .map_err(|_| Failed::with("verify_not_bound", "address", &asking.serve))?;
    let address = listener
        .local_addr()
        .map_err(|_| Failed::with("verify_not_bound", "address", &asking.serve))?;
    let scheme = match asking.under {
        Under::Nothing => "http",
        _ => "https",
    };
    let pointer = Pointer {
        verifier: asking.verifier.to_string(),
        at: format!("{scheme}://{address}{}", asking.path),
        nonce: request.nonce.clone(),
        until: network.epoch.saturating_add(OPEN_FOR),
    };
    let link = link::present(&pointer)?;
    let served = written(&request);
    log::info!(
        "verify_serving address={address} under={} path={}",
        asking.under.word(),
        asking.path
    );

    let (tell, judged) = tokio::sync::oneshot::channel();
    let serving = Arc::new(Serving {
        partner: partner.clone(),
        asking: asking.clone(),
        request,
        served: served.clone(),
        acceptor: acceptor(partner, &asking.under)?,
        until: pointer.until,
    });
    tokio::spawn(answering(listener, serving, tell));
    Ok(Started {
        address,
        link,
        pointer,
        request: served,
        judged,
    })
}

/// The request, out of the template version and what the operator asked for.
fn composed(asking: &Asking, version: &almena_store::template::Version) -> Result<Request, Failed> {
    let mut wants = Vec::with_capacity(asking.asks.len());
    for (attribute, purpose) in &asking.asks {
        let allowed = version
            .asks
            .iter()
            .find(|asked| asked.attribute == *attribute)
            .ok_or_else(|| {
                Failed::with(
                    "verify_request_malformed",
                    "not_in_template",
                    attribute.as_str(),
                )
            })?;
        wants.push(Wanted {
            attribute: attribute.clone(),
            how: allowed.how,
            required: allowed.required,
            from_credential: true,
            purpose: purpose.clone(),
        });
    }
    Ok(Request {
        template: asking.template.clone(),
        accepts: asking.accepts.clone(),
        nonce: drawn_name()?,
        audience: asking.verifier.to_string(),
        wants,
    })
}

/// What wraps a connection, where anything does.
fn acceptor(partner: &Partner, under: &Under) -> Result<Option<almena_tls::Accepting>, Failed> {
    match under {
        Under::Nothing => Ok(None),
        Under::OwnKey => {
            let keys = partner
                .directory
                .keys_held()?
                .ok_or_else(|| Failed::new("partner_no_keys"))?;
            almena_tls::self_signed(&keys.control)
                .map(Some)
                .map_err(|why| Failed::with("verify_no_certificate", "why", &format!("{why:?}")))
        }
        Under::Certificate { certificate, key } => almena_tls::accepting(certificate, key)
            .map(Some)
            .map_err(|why| Failed::with("verify_no_certificate", "why", &format!("{why:?}"))),
    }
}

/// Everything one connection needs, shared by all of them.
struct Serving {
    partner: Partner,
    asking: Asking,
    request: Request,
    served: serde_json::Value,
    acceptor: Option<almena_tls::Accepting>,
    until: u64,
}

/// Accept connections until one presentation has been judged.
async fn answering(
    listener: TcpListener,
    serving: Arc<Serving>,
    tell: tokio::sync::oneshot::Sender<Judged>,
) {
    let mut tell = Some(tell);
    while tell.is_some() {
        let Ok((io, _)) = listener.accept().await else {
            continue;
        };
        let judged = match &serving.acceptor {
            Some(acceptor) => match acceptor.accept(io).await {
                Ok(wrapped) => one(wrapped, &serving).await,
                Err(why) => {
                    log::info!("connection_not_secured reason={why}");
                    None
                }
            },
            None => one(io, &serving).await,
        };
        if let Some(judged) = judged
            && let Some(tell) = tell.take()
        {
            let _ = tell.send(judged);
        }
    }
}

/// One connection: read the request, answer it, and say whether a presentation was judged.
async fn one<S>(mut stream: S, serving: &Serving) -> Option<Judged>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let read = tokio::time::timeout(PATIENCE, request_of(&mut stream))
        .await
        .ok()??;
    let (status, body, judged) = answer(&read, serving).await;
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = tokio::time::timeout(PATIENCE, async {
        stream.write_all(response.as_bytes()).await?;
        stream.flush().await
    })
    .await;
    judged
}

/// One HTTP/1.1 request, read off the wire: method, path with its query, and body.
async fn request_of<S: AsyncRead + Unpin>(stream: &mut S) -> Option<(String, String, Vec<u8>)> {
    let mut held = Vec::new();
    let mut buffer = [0u8; 4096];
    let head_end = loop {
        if let Some(at) = held.windows(4).position(|window| window == b"\r\n\r\n") {
            break at;
        }
        if held.len() > LARGEST {
            return None;
        }
        let read = stream.read(&mut buffer).await.ok()?;
        if read == 0 {
            return None;
        }
        held.extend_from_slice(&buffer[..read]);
    };
    let head = String::from_utf8(held[..head_end].to_vec()).ok()?;
    let mut lines = head.split("\r\n");
    let mut first = lines.next()?.split_whitespace();
    let (method, target) = (first.next()?.to_owned(), first.next()?.to_owned());
    let length = lines
        .filter_map(|line| line.split_once(':'))
        .find(|(name, _)| name.trim().eq_ignore_ascii_case("content-length"))
        .and_then(|(_, value)| value.trim().parse::<usize>().ok())
        .unwrap_or(0);
    if length > LARGEST {
        return None;
    }
    let mut body = held[head_end + 4..].to_vec();
    while body.len() < length {
        let read = stream.read(&mut buffer).await.ok()?;
        if read == 0 {
            return None;
        }
        body.extend_from_slice(&buffer[..read]);
    }
    body.truncate(length);
    Some((method, target, body))
}

/// The answer to one request: status line, body, and what was judged if anything was.
async fn answer(
    read: &(String, String, Vec<u8>),
    serving: &Serving,
) -> (&'static str, String, Option<Judged>) {
    let (method, target, body) = read;
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    if path != serving.asking.path {
        return ("404 Not Found", "{}".to_owned(), None);
    }
    match method.as_str() {
        "GET" => {
            let nonce = query
                .split('&')
                .filter_map(|pair| pair.split_once('='))
                .find(|(field, _)| *field == "nonce")
                .and_then(|(_, value)| link::decoded(value));
            if nonce.as_deref() != Some(serving.request.nonce.as_str()) {
                return ("410 Gone", "{}".to_owned(), None);
            }
            log::info!("request_served nonce={}", serving.request.nonce);
            ("200 OK", serving.served.to_string(), None)
        }
        "POST" => presented(body, serving).await,
        _ => ("404 Not Found", "{}".to_owned(), None),
    }
}

/// A presentation, or a refusal, posted back.
async fn presented(body: &[u8], serving: &Serving) -> (&'static str, String, Option<Judged>) {
    let Ok(posted) = serde_json::from_slice::<serde_json::Value>(body) else {
        return ("400 Bad Request", "{}".to_owned(), None);
    };
    if posted["nonce"].as_str() != Some(serving.request.nonce.as_str()) {
        return ("410 Gone", "{}".to_owned(), None);
    }
    let judged = if posted["refused"].as_bool() == Some(true) {
        log::info!("presentation_refused nonce={}", serving.request.nonce);
        Judged {
            outcome: Outcome::NotWhatWasAsked,
            why: Some("refused".to_owned()),
        }
    } else if let Some(presentation) = posted["presentation"].as_str() {
        judge(
            &serving.partner,
            &serving.asking,
            &serving.request,
            presentation,
            serving.until,
        )
        .await
    } else {
        return ("400 Bad Request", "{}".to_owned(), None);
    };
    log::info!(
        "presentation_outcome outcome={} why={}",
        judged.outcome.word(),
        judged.why.as_deref().unwrap_or("-")
    );
    let answer = serde_json::json!({ "outcome": judged.outcome.word(), "why": judged.why });
    ("200 OK", answer.to_string(), Some(judged))
}

/// What a presentation says about where it came from, read before anything is checked.
struct Claims {
    issuer: Did,
    status: Option<(Did, u64)>,
}

/// The issuer and the status the presentation names, out of the credential's own payload.
///
/// **Read to know what to resolve, and nothing else is believed from it.** Which key checks the
/// signature and which list holds the bit both come from the record, resolved by these names.
fn claims_of(presented: &str) -> Option<Claims> {
    let parts = almena_credential::present::parts(presented)?;
    let middle = parts.jwt.split('.').nth(1)?;
    let payload: serde_json::Value =
        serde_json::from_slice(&almena_credential::base64url::decode(middle).ok()?).ok()?;
    let issuer = Did::parse(payload["iss"].as_str()?).ok()?;
    let status = match payload["status"]["revocable"].as_bool()? {
        true => Some((
            Did::parse(payload["status"]["list"].as_str()?).ok()?,
            payload["status"]["index"].as_u64()?,
        )),
        false => None,
    };
    Some(Claims { issuer, status })
}

/// Judge one presentation against the request, with everything resolved through the node.
async fn judge(
    partner: &Partner,
    asking: &Asking,
    request: &Request,
    presented: &str,
    _until: u64,
) -> Judged {
    let Some(claims) = claims_of(presented) else {
        return Judged {
            outcome: Outcome::NotWhatWasAsked,
            why: Some("malformed".to_owned()),
        };
    };
    let now = match chain::network(&partner.node).await {
        Ok(network) => network.epoch,
        Err(why) => {
            return Judged {
                outcome: Outcome::CouldNotVerify,
                why: Some(why.to_string()),
            };
        }
    };
    let resolved = resolved(partner, &claims.issuer).await;
    let revocation = match &claims.status {
        Some((list, index)) => revocation_of(partner, list, *index).await,
        None => Revocation::NothingToCheck,
    };
    let methods = [Method::Almena];
    let says = verify(
        presented,
        &Against {
            request,
            resolved: &resolved,
            policy: &Policy {
                methods: &methods,
                revocable: asking.require_revocable,
                closed_issuers: false,
            },
            revocation,
            now: Epoch::new(now),
        },
    );
    worded(says)
}

/// What the record says about the issuer, or that nobody could be asked.
async fn resolved(partner: &Partner, issuer: &Did) -> Resolved {
    let element = match chain::element(&partner.node, issuer).await {
        Ok(element) => element,
        Err(why) => {
            log::info!("issuer_unresolved issuer={issuer} reason={why}");
            return Resolved {
                issuance_key: None,
                closed: false,
            };
        }
    };
    let closed = chain::entity_closed(&partner.node, &element.of)
        .await
        .unwrap_or(false);
    let issuance_key = element.issuance.as_ref().and_then(|held| {
        <[u8; p256::PUBLIC_KEY_WIDTH]>::try_from(held.as_slice())
            .ok()
            .and_then(|bytes| p256::VerifyingKey::from_bytes(bytes).ok())
    });
    Resolved {
        issuance_key,
        closed,
    }
}

/// What is known about the bit, from the record's freshest hash and the node's copy of the bytes.
async fn revocation_of(partner: &Partner, list: &Did, index: u64) -> Revocation {
    let freshest = match chain::status_list(&partner.node, list).await {
        Ok(held) => held.latest().and_then(|version| {
            <[u8; 32]>::try_from(version.hash.as_slice())
                .ok()
                .map(almena_suite::digest::Digest::from_bytes)
        }),
        Err(why) => {
            log::info!("status_list_unresolved list={list} reason={why}");
            None
        }
    };
    let served = match freshest {
        Some(hash) => chain::list_bytes(&partner.node, hash.bytes())
            .await
            .unwrap_or_default(),
        None => None,
    };
    what_is_known(&Reached { freshest, served }, index)
}

/// The library's verdict, as the three words and a reason.
fn worded(says: Says) -> Judged {
    match says {
        Says::Proved(_) => Judged {
            outcome: Outcome::Accepted,
            why: None,
        },
        Says::DoesNotAnswer(why) => Judged {
            outcome: Outcome::NotWhatWasAsked,
            why: Some(match why {
                Unanswered::Missing(name) => format!("missing:{}", name.as_str()),
                Unanswered::Unasked(name) => format!("unasked:{name}"),
                Unanswered::PurposeMoved => "purpose_moved".to_owned(),
            }),
        },
        Says::NotValid(fault) => Judged {
            outcome: Outcome::NotWhatWasAsked,
            why: Some(fault_word(&fault).to_owned()),
        },
        Says::CouldNotVerify(missing) => Judged {
            outcome: Outcome::CouldNotVerify,
            why: Some(
                match missing {
                    Missing::IssuerUnresolved => "issuer_unresolved",
                    Missing::StatusStale => "status_stale",
                    Missing::StatusUnavailable => "status_unavailable",
                }
                .to_owned(),
            ),
        },
    }
}

/// A fault, as the word the records carry.
const fn fault_word(fault: &Fault) -> &'static str {
    match fault {
        Fault::Malformed => "malformed",
        Fault::MethodNotAccepted => "method_not_accepted",
        Fault::ProofUnknown => "proof_unknown",
        Fault::BadSignature => "bad_signature",
        Fault::Expired => "expired",
        Fault::NotYetIssued => "not_yet_issued",
        Fault::IssuerClosed => "issuer_closed",
        Fault::WrongTemplate => "wrong_template",
        Fault::NotRevocable => "not_revocable",
        Fault::Revoked => "revoked",
        Fault::NotCommitted => "not_committed",
        Fault::NotBound => "not_bound",
        Fault::WrongChallenge => "wrong_challenge",
        Fault::BindingDoesNotCover => "binding_does_not_cover",
        Fault::BindingFailed => "binding_failed",
    }
}

/// One `--ask attribute[:purpose]` as the command line writes it.
///
/// # Errors
///
/// `verify_ask_unreadable` for an attribute that is not a name or a purpose that is empty.
pub fn ask(written: &str) -> Result<(Name, String), Failed> {
    let (attribute, purpose) = written
        .split_once(':')
        .ok_or_else(|| Failed::with("verify_ask_unreadable", "given", written))?;
    let attribute = Name::parse(attribute.trim())
        .map_err(|_| Failed::with("verify_ask_unreadable", "attribute", attribute.trim()))?;
    if purpose.trim().is_empty() {
        return Err(Failed::with("verify_ask_unreadable", "purpose", "empty"));
    }
    Ok((attribute, purpose.trim().to_owned()))
}

#[cfg(test)]
mod tests {
    use super::{Outcome, Under, ask, claims_of, worded};
    use almena_credential::verify::{Fault, Missing};
    use almena_format::identifier::Name;
    use almena_sdk::verifier::{Says, Unanswered};

    #[test]
    fn the_three_answers_stay_three_and_each_carries_its_reason() {
        assert_eq!(
            worded(Says::NotValid(Fault::Revoked)).why.as_deref(),
            Some("revoked")
        );
        assert_eq!(
            worded(Says::NotValid(Fault::Revoked)).outcome,
            Outcome::NotWhatWasAsked
        );
        let missing = worded(Says::CouldNotVerify(Missing::StatusStale));
        assert_eq!(missing.outcome, Outcome::CouldNotVerify);
        assert_eq!(missing.why.as_deref(), Some("status_stale"));
        let short = worded(Says::DoesNotAnswer(Unanswered::Missing(Name::of(b"x"))));
        assert!(
            short
                .why
                .as_deref()
                .is_some_and(|why| why.starts_with("missing:z"))
        );
        assert_eq!(Outcome::Accepted.word(), "accepted");
        assert_eq!(Under::Nothing.word(), "nothing");
    }

    #[test]
    fn an_ask_is_an_attribute_and_a_purpose_and_never_one_without_the_other() {
        let name = Name::of(b"an attribute");
        let (attribute, purpose) =
            ask(&format!("{}:to sell something", name.as_str())).expect("read");
        assert_eq!(attribute, name);
        assert_eq!(purpose, "to sell something");
        assert!(ask(name.as_str()).is_err());
        assert!(ask(&format!("{}:  ", name.as_str())).is_err());
        assert!(ask("nope:why").is_err());
    }

    #[test]
    fn what_a_presentation_claims_about_its_issuer_is_read_and_nothing_is_believed() {
        assert!(claims_of("").is_none());
        assert!(claims_of("a.b.c~~").is_none(), "the middle is not JSON");
    }
}
