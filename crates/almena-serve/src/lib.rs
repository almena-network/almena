//! What carries a question to a node and an answer back, and decides nothing on the way.
//!
//! Everything about *what* a node says lives one crate over, where nothing knows there is a
//! network. This holds sockets, methods, paths and status codes, and its whole job is to hand what
//! arrives to that crate and write down what comes back. **It computes no answer.** If it ever
//! did, there would be two places a node's behaviour lives, and the two would drift.
//!
//! # HTTP describes the request; the body describes the object
//!
//! This is the one decision here worth arguing about, and it goes the way it does on purpose.
//!
//! *Does not exist*, *cannot resolve*, *not here*, *here*, *taken* and *not taken* are all **200**.
//! They are answers, and which one it is, is in the body. The pull toward `404` for *does not
//! exist* is almost irresistible and it would be a mistake: `404` already means *there is nothing
//! at that path*, so using it for both makes *this node serves no such question* and *no such
//! object was ever created* the same reply — and those are not remotely the same thing to whoever
//! asked.
//!
//! So: **`404` is about the path, `400` is about the request, `429` is about a limit, and
//! everything about the object is `200` with the answer inside.**
//!
//! # TLS is not here
//!
//! Serving takes a stream and does not care what it is. Whoever terminates TLS wraps the stream
//! and passes it in — which means the second cryptographic implementation TLS needs enters with
//! the face that needs it rather than with every node that links this.

use std::sync::Arc;

use almena_api::{Limits, Said, State, Unreadable, answer, deliver, parse, throttled, unreadable};
use almena_node::{Keeping, Node};
use almena_time::Epoch;
use http_body_util::{BodyExt as _, Full, Limited};
use hyper::body::{Bytes, Incoming};
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::server::conn::auto;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::{OwnedSemaphorePermit, RwLock, Semaphore};

/// Where an act is handed over.
///
/// Reading and writing are not two arms of one thing here: one takes the node by shared reference
/// and the other needs it exclusively, and the types say so.
const ACTS: &str = "/acts";

/// Where a device talks to its own mediator: declaring, taking, confirming.
///
/// **Only on a node that runs a mailbox**, which is what the node itself answers. This layer does
/// not decide it and does not know it — a transport that could switch a capability on would be a
/// second place where what a node offers is decided, and the two would drift.
const POST: &str = "/post";

/// Where a message is left for somebody, under their identifier.
const POST_TO: &str = "/post/";

/// One node, answering.
///
/// Cheap to clone, because every connection holds one and they must all reach the **same** node: a
/// node is a directory with a key in it, and two of anything over one directory is a conflict.
#[derive(Debug, Clone)]
pub struct Serving {
    /// The node itself. Shared, because both faces and every connection reach the same one — a
    /// node is a directory with a key in it, and two of anything over one directory is a conflict.
    node: Arc<RwLock<Node>>,
    /// What this node will and will not do, published as an answer like any other.
    limits: Limits,
    /// How many connections are held at once, across everybody.
    ///
    /// **The only limit that bounds anything.** Per-connection limits cannot: connections times
    /// requests times bytes has no ceiling. It is also why *try again from another connection* is
    /// not on its own a test for censorship — this number is what lets somebody tell the two
    /// apart, which is why it is published rather than kept.
    holding: Arc<Semaphore>,
}

impl Serving {
    /// A node, ready to answer, holding to `limits`.
    #[must_use]
    pub fn new(node: Node, limits: Limits) -> Self {
        let connections = usize::try_from(limits.connections).unwrap_or(usize::MAX);
        Self {
            node: Arc::new(RwLock::new(node)),
            limits,
            holding: Arc::new(Semaphore::new(connections)),
        }
    }

    /// The node this is serving.
    ///
    /// Handed out rather than wrapped away, because a face draws the same node it serves: one that
    /// had to be given up in order to answer questions would be one the face could no longer show.
    #[must_use]
    pub fn node(&self) -> &Arc<RwLock<Node>> {
        &self.node
    }

    /// What this node announced it will do.
    #[must_use]
    pub fn limits(&self) -> &Limits {
        &self.limits
    }

    /// A place for one more connection, if this node has one.
    ///
    /// [`None`] means it is holding as many as it said it would. Holding the permit is what keeps
    /// the place: dropping it gives it back, so a connection that ends any way at all — answered,
    /// abandoned, or failed — frees it without anybody remembering to.
    #[must_use]
    pub fn room(&self) -> Option<OwnedSemaphorePermit> {
        Arc::clone(&self.holding).try_acquire_owned().ok()
    }

    /// Answer on `io` until whoever is on the other end is done.
    ///
    /// `clock` says what epoch it is. It is asked each time rather than captured once, so that a
    /// connection held open across an epoch boundary answers with the epoch it is in and not the
    /// one it arrived in.
    ///
    /// # Errors
    ///
    /// Whatever the connection did wrong. It is worth nothing to a caller beyond writing down, and
    /// is not a reason to stop answering anybody else.
    pub async fn connection<I, C>(
        &self,
        io: I,
        clock: C,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
    where
        I: AsyncRead + AsyncWrite + Unpin + Send + 'static,
        C: Fn() -> Epoch + Clone + Send + Sync + 'static,
    {
        let asked = Arc::new(tokio::sync::Mutex::new(Asking::new()));

        auto::Builder::new(TokioExecutor::new())
            .serve_connection(
                TokioIo::new(io),
                service_fn(move |request| {
                    let serving = self.clone();
                    let clock = clock.clone();
                    let asked = Arc::clone(&asked);
                    async move { serving.one(request, &clock, &asked).await }
                }),
            )
            .await
    }

    /// One request, answered.
    async fn one<C>(
        &self,
        request: Request<Incoming>,
        clock: &C,
        asked: &Arc<tokio::sync::Mutex<Asking>>,
    ) -> Result<Response<Full<Bytes>>, std::convert::Infallible>
    where
        C: Fn() -> Epoch,
    {
        let now = clock();

        // Counted before anything is read, because the point of a limit is to stop work rather
        // than to be applied to work already done.
        if !asked
            .lock()
            .await
            .room(tokio::time::Instant::now(), &self.limits)
        {
            let node = self.node.read().await;
            return Ok(written(&throttled(&node, now, &self.limits)));
        }

        let method = request.method().clone();
        let path = request.uri().path().to_owned();

        if method == Method::POST && path == ACTS {
            let largest = usize::try_from(self.limits.largest_act).unwrap_or(usize::MAX);
            let body = Limited::new(request.into_body(), largest).collect().await;
            let Ok(body) = body else {
                // Over the size this node announced, or the body ended early. Either way there is
                // no act here to be taken.
                let node = self.node.read().await;
                return Ok(written(&unreadable(&node, now, Unreadable::Malformed)));
            };
            let mut node = self.node.write().await;
            return Ok(written(&deliver(&mut node, &body.to_bytes(), now)));
        }

        if method == Method::POST && (path == POST || path.starts_with(POST_TO)) {
            return Ok(written(&self.posted(request, &path, now).await));
        }

        let node = self.node.read().await;
        let said = match parse(method.as_str(), &path) {
            Ok(ask) => answer(&node, &ask, now, &self.limits),
            Err(why) => unreadable(&node, now, why),
        };
        Ok(written(&said))
    }
}

impl Serving {
    /// One request to the mailbox, answered.
    ///
    /// Split out from `one` because it is the only other path with a body, and the body is read
    /// under the same ceiling an act is: a mediator that would read any size at all is a mediator
    /// anybody can exhaust before a quota ever gets a chance to say no.
    async fn posted(&self, request: Request<Incoming>, path: &str, now: Epoch) -> Said {
        let largest = usize::try_from(self.limits.largest_act).unwrap_or(usize::MAX);
        let Ok(body) = Limited::new(request.into_body(), largest).collect().await else {
            let node = self.node.read().await;
            return unreadable(&node, now, Unreadable::Malformed);
        };
        let body = body.to_bytes();

        // **Whatever the sender was given**, which is a relationship's own address rather than an
        // account's (`SPECS.md §6.5`). This layer does not read it and does not know what it names:
        // which of its customers answers to an address is the node's question, and a transport that
        // decided it would be a second place the answer lives.
        let to = path.strip_prefix(POST_TO).map(str::to_owned);
        if to
            .as_ref()
            .is_some_and(|to| to.is_empty() || to.contains('/'))
        {
            let node = self.node.read().await;
            return unreadable(&node, now, Unreadable::Malformed);
        }

        let mut node = self.node.write().await;
        match to {
            Some(to) => almena_api::post::deliver(&mut node, &to, &body, now),
            None => almena_api::post::asked(&mut node, &body, now),
        }
    }
}

/// What one connection has asked for, and since when.
///
/// Per connection and not per node: the number that bounds a node is how many connections it holds
/// at once, and this is the one that keeps a single conversation from being the whole of it.
///
/// # It measures against a clock, and that is not the rule the rest of this platform follows
///
/// Nothing that decides whether an act is **valid** may read a clock — validity is settled against
/// the epoch it is handed, so that every rule is testable at any moment in a network's life. This
/// is not that. How often one socket may ask is bookkeeping about a conversation, not a fact about
/// the record, and nothing it decides survives the connection.
///
/// It has to be a real duration because **that is what the node announced**. A window published as
/// sixty seconds and enforced by the hour would make what a node said and what it did two
/// different things, which is the one property publishing the limits exists to give away.
#[derive(Debug)]
struct Asking {
    /// How many questions have been asked in this window.
    asked: u64,
    /// When the window started. [`None`] until the first question.
    since: Option<tokio::time::Instant>,
}

impl Asking {
    /// A connection that has asked nothing.
    const fn new() -> Self {
        Self {
            asked: 0,
            since: None,
        }
    }

    /// Whether there is room for one more question, counting it if there is.
    fn room(&mut self, at: tokio::time::Instant, limits: &Limits) -> bool {
        let window = std::time::Duration::from_secs(limits.window);
        let over = self
            .since
            .is_none_or(|start| at.duration_since(start) >= window);
        if over {
            self.since = Some(at);
            self.asked = 0;
        }

        if self.asked >= limits.per_connection {
            return false;
        }
        self.asked += 1;
        true
    }
}

/// Close whatever epochs are owed up to `now`, and say how many that was.
///
/// A node publishes a root every epoch whether or not anything happened, so one that was off for
/// three epochs owes three roots on its return and not one: a gap that could mean either *nothing
/// happened* or *I was not here* means neither.
pub async fn catch_up(serving: &Serving, keeping: &mut Keeping, now: Epoch) -> usize {
    let owed = keeping.due(now);
    if owed.is_empty() {
        return 0;
    }
    // The node decides what each of them is over. An epoch it is closing late gets the tree it last
    // put its name to, because whatever arrived since, it observed now.
    serving.node.write().await.close_owed(&owed)
}

/// A node's clock, kept in one place so that a timer and a person cannot disagree.
///
/// **What is owed is one fact, not two.** A timer closing epochs on its own schedule and somebody
/// asking for one to be closed now are the same question asked twice, and answering it from two
/// records would mean a node that had just been told to catch up would do it again a minute later
/// — publishing an epoch it had already spoken for, which is the one thing it must never do.
///
/// So both go through this, and it is cheap to hold: a clone shares the record rather than copying
/// it.
#[derive(Debug, Clone, Default)]
pub struct Timekeeping {
    /// What has been closed, shared by everything that closes anything.
    keeping: Arc<tokio::sync::Mutex<Keeping>>,
}

impl Timekeeping {
    /// A clock that has closed nothing.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Close whatever is owed up to `now`, and say how many that was.
    ///
    /// Zero is an answer and not a failure: it means this node is up to date, which is what it
    /// should be most of the time somebody asks.
    pub async fn catch_up(&self, serving: &Serving, now: Epoch) -> usize {
        let mut keeping = self.keeping.lock().await;
        catch_up(serving, &mut keeping, now).await
    }

    /// The last epoch closed, if any has been.
    pub async fn closed(&self) -> Option<Epoch> {
        self.keeping.lock().await.closed()
    }

    /// Keep closing what is owed, for as long as this is polled.
    ///
    /// It belongs to **being on a network**, not to answering questions: an epoch is owed whether
    /// or not anybody is asking, and a node whose clock only ran while its interface was up would
    /// leave gaps that mean *nothing happened* and *I was not here* at the same time.
    pub async fn keeping_time<C>(self, serving: Serving, clock: C, every: std::time::Duration)
    where
        C: Fn() -> Epoch + Send + 'static,
    {
        loop {
            self.catch_up(&serving, clock()).await;
            tokio::time::sleep(every).await;
        }
    }
}

/// What an answer looks like once it is on the wire.
///
/// The only decision here is which status code carries it, and it is a decision about the
/// *request* rather than about what was asked after. Everything about the object rides in the
/// body, where a reader has to look anyway.
fn written(said: &Said) -> Response<Full<Bytes>> {
    let status = match said.state {
        State::NoSuchQuestion => StatusCode::NOT_FOUND,
        State::Malformed => StatusCode::BAD_REQUEST,
        State::Throttled => StatusCode::TOO_MANY_REQUESTS,
        // Everything else is an answer, and which answer it is, is inside.
        _ => StatusCode::OK,
    };

    let mut response = Response::new(Full::new(Bytes::from(said.body.clone())));
    *response.status_mut() = status;
    response.headers_mut().insert(
        hyper::header::CONTENT_TYPE,
        hyper::header::HeaderValue::from_static("application/cbor"),
    );
    response
}

#[cfg(test)]
mod tests {
    use super::{ACTS, Serving, written};
    use almena_api::{Limits, Said, State};
    use almena_format::cbor::{Value, read};
    use almena_format::identifier::{Name, Network};
    use almena_format::operation::{Signed, create};
    use almena_node::Node;
    use almena_store::genesis::Which;
    use almena_store::kind::Kind;
    use almena_suite::ed25519;
    use almena_time::Epoch;
    use hyper::StatusCode;
    use std::collections::BTreeMap;

    /// A development network with a fixed clock, so that a test is never about what time it is.
    fn at() -> almena_node::Opening {
        almena_node::Opening {
            which: Which::Development,
            beginning: Epoch::GENESIS,
            began: 1_800_000_000,
        }
    }

    fn key(seed: u8) -> ed25519::SigningKey {
        ed25519::SigningKey::from_secret([seed; 32])
    }

    fn limits() -> Limits {
        Limits {
            per_connection: 60,
            window: 60,
            largest_act: 65_536,
            connections: 2,
        }
    }

    fn serving() -> Serving {
        let node = Node::open(&at(), &[], &key(5), key(6)).expect("nobody to join");
        Serving::new(node, limits())
    }

    fn an_account(control: &ed25519::SigningKey) -> Vec<u8> {
        let public = control.verifying_key().bytes();
        let mut operation = create(
            Network::Development,
            Kind::HOLDER_CREATE.number(),
            1,
            Epoch::GENESIS,
            BTreeMap::from([(1, Value::Bytes(public.to_vec()))]),
        );
        let signature = control.sign(&operation.signing_bytes());
        operation.signatures.push(Signed {
            by: operation.object.clone(),
            key: public.to_vec(),
            signature: signature.bytes(),
        });
        operation.to_bytes()
    }

    /// Ask over a real socket, and get back what came out of it.
    async fn asked(serving: &Serving, method: &str, path: &str, body: Vec<u8>) -> (u16, Vec<u8>) {
        use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("a port");
        let address = listener.local_addr().expect("an address");

        let served = serving.clone();
        let server = tokio::spawn(async move {
            let (io, _) = listener.accept().await.expect("a connection");
            let _ = served.connection(io, || Epoch::GENESIS).await;
        });

        let mut stream = tokio::net::TcpStream::connect(address)
            .await
            .expect("connected");
        let head = format!(
            "{method} {path} HTTP/1.1\r\nHost: node\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        stream.write_all(head.as_bytes()).await.expect("sent");
        stream.write_all(&body).await.expect("sent");

        let mut raw = Vec::new();
        stream.read_to_end(&mut raw).await.expect("read");
        server.await.expect("served");

        let split = raw
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .expect("a complete response");
        let status = String::from_utf8_lossy(&raw[..split])
            .split_whitespace()
            .nth(1)
            .and_then(|code| code.parse().ok())
            .expect("a status");

        (status, raw[split + 4..].to_vec())
    }

    /// What the node said happened, read out of the response.
    fn state(body: &[u8]) -> u64 {
        let Ok(Value::Map(fields)) = read(body) else {
            panic!("a response is a canonical map");
        };
        let Some(&Value::Uint(state)) = fields.get(&3) else {
            panic!("every response says what happened");
        };
        state
    }

    #[tokio::test]
    async fn a_question_travels_over_a_real_socket_and_comes_back_answered() {
        let (status, body) = asked(&serving(), "GET", "/limits", Vec::new()).await;
        assert_eq!(status, 200);
        assert_eq!(state(&body), State::Here as u64);
    }

    #[tokio::test]
    async fn an_act_handed_over_is_written_down() {
        let serving = serving();
        let before = serving.node().read().await.written();

        let (status, body) = asked(&serving, "POST", ACTS, an_account(&key(9))).await;
        assert_eq!(status, 200);
        assert_eq!(state(&body), State::Taken as u64);
        assert_eq!(serving.node().read().await.written(), before + 1);
    }

    #[tokio::test]
    async fn nothing_at_a_path_is_not_the_same_as_nothing_at_that_name() {
        // The distinction the status codes exist to keep: `404` means this node serves no such
        // question, and it must never also mean *no such object was ever created*. Those are not
        // remotely the same thing to whoever asked.
        let serving = serving();

        let (status, _) = asked(&serving, "GET", "/nothing-served-here", Vec::new()).await;
        assert_eq!(status, StatusCode::NOT_FOUND.as_u16());

        let name = Name::of(b"never created");
        let (status, body) = asked(
            &serving,
            "GET",
            &format!("/object/{}", name.as_str()),
            Vec::new(),
        )
        .await;
        assert_eq!(status, 200, "the question was served");
        assert_eq!(
            state(&body),
            State::DoesNotExist as u64,
            "and this is its answer"
        );
    }

    #[tokio::test]
    async fn something_unreadable_in_the_path_is_about_the_request() {
        let (status, body) = asked(&serving(), "GET", "/object/not-a-name", Vec::new()).await;
        assert_eq!(status, StatusCode::BAD_REQUEST.as_u16());
        assert_eq!(state(&body), State::Malformed as u64);
    }

    #[tokio::test]
    async fn bytes_that_are_not_an_act_are_a_bad_request() {
        // Nothing was read, so there is nothing to answer *about*. That really is a fault in the
        // request and the status says so.
        let serving = serving();
        let (status, body) = asked(&serving, "POST", ACTS, b"not an act at all".to_vec()).await;
        assert_eq!(status, StatusCode::BAD_REQUEST.as_u16());
        assert_eq!(state(&body), State::Malformed as u64);
    }

    #[tokio::test]
    async fn an_act_that_was_read_and_refused_is_an_answer_and_not_a_bad_request() {
        // The distinction that matters on this path: the node understood it perfectly and will
        // not write it down. That is something it has to say *about the act*, so it says it in the
        // body — which rule was broken — and the request itself was fine.
        let serving = serving();

        // The same account twice. The second is a well-formed act that breaks a rule.
        let act = an_account(&key(9));
        let (first, _) = asked(&serving, "POST", ACTS, act.clone()).await;
        assert_eq!(first, 200);

        let (status, body) = asked(&serving, "POST", ACTS, act).await;
        assert_eq!(status, 200, "the request was fine");
        assert_eq!(state(&body), State::NotTaken as u64, "the act was not");

        let Ok(Value::Map(fields)) = read(&body) else {
            panic!("a response is a canonical map");
        };
        assert!(fields.contains_key(&5), "and it says which rule");
    }

    #[test]
    fn only_three_things_are_ever_about_the_request() {
        // Everything else is an answer about an object, and putting any of those on a status code
        // would collapse two different facts into one reply.
        let answers = [
            (State::Here, StatusCode::OK),
            (State::DoesNotExist, StatusCode::OK),
            (State::CannotResolve, StatusCode::OK),
            (State::NotHere, StatusCode::OK),
            (State::NotTaken, StatusCode::OK),
            (State::Taken, StatusCode::OK),
            (State::NotYetAskable, StatusCode::OK),
            (State::NoSuchQuestion, StatusCode::NOT_FOUND),
            (State::Malformed, StatusCode::BAD_REQUEST),
            (State::Throttled, StatusCode::TOO_MANY_REQUESTS),
        ];
        for (state, expected) in answers {
            let said = Said {
                state,
                body: Vec::new(),
            };
            assert_eq!(written(&said).status(), expected, "{state:?}");
        }
    }

    #[tokio::test]
    async fn a_node_that_was_away_comes_back_owing_every_epoch_it_missed() {
        // And closes all of them, so that its own record of holes comes back empty. Publishing
        // only the epoch it woke up in would leave a gap meaning either *nothing happened* or
        // *I was not here*.
        use almena_node::Keeping;
        use almena_time::Epochs;

        let serving = serving();
        let mut keeping = Keeping::new();

        assert_eq!(
            super::catch_up(&serving, &mut keeping, Epoch::GENESIS).await,
            1,
            "the epoch it started in"
        );

        let awake = Epoch::GENESIS.plus(Epochs(4)).expect("no overflow");
        assert_eq!(
            super::catch_up(&serving, &mut keeping, awake).await,
            4,
            "the four it was away for"
        );
        assert_eq!(
            super::catch_up(&serving, &mut keeping, awake).await,
            0,
            "and nothing twice"
        );

        assert!(serving.node().read().await.missing(awake).is_empty());
    }

    #[test]
    fn a_connection_gets_as_many_questions_as_the_node_announced() {
        // What a node published and what it does have to be the same fact, or publishing it buys
        // nobody anything.
        let limits = limits();
        let mut asking = super::Asking::new();
        let start = tokio::time::Instant::now();

        for question in 0..limits.per_connection {
            assert!(asking.room(start, &limits), "question {question}");
        }
        assert!(
            !asking.room(start, &limits),
            "and the one after the number it announced does not fit"
        );
    }

    #[test]
    fn the_window_is_the_one_that_was_announced_and_not_an_epoch() {
        // The mistake worth a test: an epoch is an hour, and a window published in seconds that
        // was enforced by the hour would be a node doing something other than what it said.
        let limits = limits();
        let mut asking = super::Asking::new();
        let start = tokio::time::Instant::now();

        for _ in 0..limits.per_connection {
            assert!(asking.room(start, &limits));
        }
        assert!(!asking.room(start, &limits));

        let just_inside = start + std::time::Duration::from_secs(limits.window - 1);
        assert!(!asking.room(just_inside, &limits), "still the same window");

        let after = start + std::time::Duration::from_secs(limits.window);
        assert!(asking.room(after, &limits), "and the next one is fresh");
    }

    #[test]
    fn a_connection_that_has_asked_nothing_is_not_over_its_limit() {
        // A window that started at zero rather than at the first question would refuse the very
        // first thing anybody asked on a long-lived connection.
        let mut asking = super::Asking::new();
        assert!(asking.room(tokio::time::Instant::now(), &limits()));
    }

    #[tokio::test]
    async fn asking_twice_is_not_two_answers_about_one_epoch() {
        // The reason a timer and a person share one record. If they kept their own, a node that
        // had just been told to catch up would do it again when the timer came round — publishing
        // an epoch it had already spoken for, which is the one thing it must never do.
        let serving = serving();
        let timekeeping = super::Timekeeping::new();
        let now = Epoch::GENESIS
            .plus(almena_time::Epochs(2))
            .expect("no overflow");

        // One, not three: a node that has only just started was not absent for the epochs before
        // it existed, and claiming them would be publishing a history it never observed.
        assert_eq!(timekeeping.catch_up(&serving, now).await, 1);
        assert_eq!(
            timekeeping.catch_up(&serving, now).await,
            0,
            "and nothing is owed twice"
        );
    }

    #[tokio::test]
    async fn the_timer_and_a_person_are_looking_at_one_record() {
        // Two clones of the clock are the same clock, which is what makes it safe for the timer to
        // hold one while a face holds another.
        let serving = serving();
        let timer = super::Timekeeping::new();
        let person = timer.clone();

        assert_eq!(timer.catch_up(&serving, Epoch::GENESIS).await, 1);
        assert_eq!(
            person.catch_up(&serving, Epoch::GENESIS).await,
            0,
            "the other one had already closed it"
        );
        assert_eq!(person.closed().await, Some(Epoch::GENESIS));
    }

    #[tokio::test]
    async fn closing_on_demand_leaves_no_gap_behind_it() {
        // What somebody asking is actually asking for: not *close this one*, but *be up to date*.
        // Once the clock has started, every epoch from there on is spoken for — a node that
        // skipped to the epoch it was asked about would leave a hole meaning both `nothing
        // happened` and `I was not here`.
        let serving = serving();
        let timekeeping = super::Timekeeping::new();

        timekeeping.catch_up(&serving, Epoch::GENESIS).await;
        let through = Epoch::GENESIS
            .plus(almena_time::Epochs(4))
            .expect("no overflow");
        assert_eq!(timekeeping.catch_up(&serving, through).await, 4);

        assert!(
            serving.node().read().await.missing(through).is_empty(),
            "every epoch up to it is spoken for"
        );
    }

    #[tokio::test]
    async fn a_node_holds_only_as_many_connections_as_it_said_it_would() {
        // And that number is published, because a node that sheds a connection behaves exactly
        // like one that is censoring unless somebody can check what it announced.
        let serving = serving();
        let first = serving.room().expect("room");
        let second = serving.room().expect("room");
        assert!(serving.room().is_none(), "two was the number it announced");

        drop(first);
        assert!(serving.room().is_some(), "and it lets go again");
        drop(second);
    }

    #[tokio::test]
    async fn the_mailbox_paths_reach_the_node_and_a_node_without_one_says_so() {
        // **What this layer is allowed to decide about the mailbox is nothing.** It reads an
        // identifier out of a path and hands the body over; whether there is a mailbox at all is
        // the node's own announcement to make, and a node that has not made it answers *there is
        // nothing at that path* — which is what comes back here, off the wire, unaltered.
        let serving = serving();
        let nobody = almena_format::identifier::Did::new(Network::Development, Name::of(b"nobody"));
        for path in ["/post".to_owned(), format!("/post/{nobody}")] {
            let (status, body) = asked(&serving, "POST", &path, b"anything at all".to_vec()).await;
            assert_eq!(status, 404, "there is nothing at that path on this node");
            let Ok(Value::Map(fields)) = read(&body) else {
                panic!("a response is a canonical map");
            };
            assert_eq!(
                fields.get(&3),
                Some(&Value::Uint(State::NoSuchQuestion as u64)),
                "{path}"
            );
            assert!(
                fields.contains_key(&1) && fields.contains_key(&2),
                "stamped"
            );
        }

        // **And an address this layer cannot make sense of is still the node's to answer.** What a
        // sender holds is a relationship's own address rather than an account's (`SPECS.md §6.5`),
        // and which of its customers answers to one is a question only the node can put — so the
        // transport carries whatever it was given and decides nothing about it.
        let (_, body) = asked(&serving, "POST", "/post/whatever-this-is", Vec::new()).await;
        let Ok(Value::Map(fields)) = read(&body) else {
            panic!("a response is a canonical map");
        };
        assert_eq!(
            fields.get(&3),
            Some(&Value::Uint(State::NoSuchQuestion as u64)),
            "this node runs no mailbox, which is what it says rather than judging the address"
        );

        // What it does refuse is a path that is not one address: two segments would be two
        // questions, and reading the first as the whole would be answering a different one.
        let (status, _) = asked(&serving, "POST", "/post/one/another", Vec::new()).await;
        assert_eq!(status, 400);
    }

    #[tokio::test]
    async fn every_answer_off_the_wire_carries_its_stamp() {
        // What the transport must never be allowed to strip.
        let serving = serving();
        for (method, path) in [
            ("GET", "/limits"),
            ("GET", "/nothing-served-here"),
            ("GET", "/object/not-a-name"),
            ("GET", "/state/did:almena:dev:nobody"),
            ("GET", "/kept/0"),
            ("GET", "/capacity"),
            ("GET", "/catalogue"),
            ("POST", "/post"),
        ] {
            let (_, body) = asked(&serving, method, path, Vec::new()).await;
            let Ok(Value::Map(fields)) = read(&body) else {
                panic!("a response is a canonical map");
            };
            assert!(fields.contains_key(&1), "{path} carried no epoch");
            assert!(fields.contains_key(&2), "{path} carried no root");
        }
    }
}
