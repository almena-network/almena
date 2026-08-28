//! What two nodes say to each other, and in what bytes.
//!
//! Two questions and no more. A node that has fallen behind asks **what came after where I got
//! to**; a node that has heard of an act it does not hold asks **for that act by its name**. Both
//! answers are somebody else's signed acts, handed on exactly as they arrived.
//!
//! # Receiving is not believing
//!
//! Nothing that arrives here is trusted for having arrived. An act pulled from another node goes
//! through the same admission as one handed over an interface by a stranger, because it *is* one:
//! the signature is the authorisation and the sender is nobody. A node that accepted acts because
//! of who sent them would be a node whose record depends on its neighbours being honest, which is
//! the property this whole design exists not to need.
//!
//! So the worst a hostile node can do down this protocol is **not answer**, answer slowly, or leave
//! things out. It cannot make anything up.
//!
//! # By position, not by time
//!
//! A node asks for what came after a position in **its own** record of what that node had. No
//! clock, no agreement about one, and nothing to disagree about — where two nodes are up to is a
//! fact each of them holds separately, which is what makes catching up work between machines that
//! agree about nothing else.

use almena_format::cbor::Value;
use almena_format::identifier::Name;
use libp2p::futures;

/// Where the kind of question sits.
///
/// Odd, and there is nothing to weigh: a reader that skipped it would hold a question without
/// knowing which one.
const ASKING: u64 = 1;

/// Where the position sits, when the question is about one.
const FROM: u64 = 3;

/// Where the name sits, when the question is about one act.
const NAMED: u64 = 5;

/// Where the acts sit in an answer.
const ACTS: u64 = 1;

/// Where a node says how much it has written down.
///
/// Odd, because without it an answer that came back short is indistinguishable from the end of the
/// record — and a node that stopped asking there would sit quietly out of date.
const WRITTEN: u64 = 3;

/// The question that asks for what came after a position.
const SINCE: u64 = 1;

/// The question that asks for one act by name.
const ONE: u64 = 2;

/// The question that asks what a node signed about an epoch.
const ROOT: u64 = 3;

/// Where the epoch sits, when the question is about one.
const EPOCH: u64 = 7;

/// The thing said when a node has seen somebody else's root.
const SAW: u64 = 4;

/// The thing said when a node has written more down than it had.
const GROWN: u64 = 5;

/// Where the position sits, when the thing said is about how far a node has got.
const REACHED: u64 = 13;

/// Where the witness's own signature sits.
const SIGNED: u64 = 11;

/// Where a signed root sits in an answer.
///
/// Even, and deliberately: an answer that carries one where the asker only wanted acts is an answer
/// the asker may ignore, and one that carries none where a root was asked for is a node saying it
/// has nothing to say about that epoch.
const ROOT_SAID: u64 = 2;

/// What one node asks another.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ask {
    /// Everything written down after this position.
    Since(u64),
    /// One act, by the name it is called.
    Act(Name),
    /// What that node signed about an epoch.
    ///
    /// **The question finality is counted from.** An act is as firm as the number of independent
    /// trees that carry it, and this is how a node collects the roots to count.
    Root(u64),
    /// **Not a question either.** One node telling another that its record has grown.
    ///
    /// **It carries nothing to believe.** The number is a hint about where to ask from, and
    /// everything that actually moves still moves by being asked for and admitted like anything
    /// else — so the worst a liar can do down this is make somebody ask a question, which costs a
    /// round trip and buys them nothing.
    ///
    /// Without it an act reaches the next node when that node next happens to ask, which is a wait
    /// measured in whatever interval somebody chose. With it, telling is what starts the asking.
    Grown(u64),
    /// **Not a question.** One node telling another that it saw the root it published for an
    /// epoch, and signing the same bytes to say so.
    ///
    /// It travels the same way because there is nothing else to travel on, and because saying it
    /// and being told it was heard are two halves of one thing. What it buys is that a node cannot
    /// quietly show one root to one person and another to somebody else: the two would carry
    /// different witnesses, and the pair is the proof.
    Saw(u64, [u8; 64]),
}

/// What comes back.
///
/// The acts are in the bytes their authors signed. Nothing is re-encoded on the way, because the
/// name of an act is the hash of those bytes and a well-meant tidy-up would rename it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Said {
    /// The acts, in order, as they were written down.
    pub acts: Vec<Vec<u8>>,
    /// How much the answering node has written down altogether.
    ///
    /// What tells a short answer apart from the end of the record.
    pub written: u64,
    /// What that node signed about the epoch it was asked about, if it has closed one.
    ///
    /// [`None`] is *I have said nothing about that epoch*, which is an answer and not a failure: a
    /// node that has not got there yet has nothing to say, and inventing something would be worse
    /// than saying so.
    pub root: Option<Vec<u8>>,
}

/// Why something on the wire could not be read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Unreadable;

impl Ask {
    /// The bytes of this question.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let map = match self {
            Self::Since(from) => [(ASKING, Value::Uint(SINCE)), (FROM, Value::Uint(*from))]
                .into_iter()
                .collect(),
            Self::Act(name) => [
                (ASKING, Value::Uint(ONE)),
                (NAMED, Value::Text(name.as_str().to_owned())),
            ]
            .into_iter()
            .collect(),
            Self::Root(epoch) => [(ASKING, Value::Uint(ROOT)), (EPOCH, Value::Uint(*epoch))]
                .into_iter()
                .collect(),
            Self::Saw(epoch, signature) => [
                (ASKING, Value::Uint(SAW)),
                (EPOCH, Value::Uint(*epoch)),
                (SIGNED, Value::Bytes(signature.to_vec())),
            ]
            .into_iter()
            .collect(),
            Self::Grown(written) => [
                (ASKING, Value::Uint(GROWN)),
                (REACHED, Value::Uint(*written)),
            ]
            .into_iter()
            .collect(),
        };
        Value::Map(map).to_bytes()
    }

    /// A question read back off the wire.
    ///
    /// # Errors
    ///
    /// [`Unreadable`] for anything that is not one of the two questions, in canonical bytes, with
    /// what that question needs. There is nothing to be lenient about: a question this build cannot
    /// read is one it must not guess at.
    pub fn read(bytes: &[u8]) -> Result<Self, Unreadable> {
        let Ok(Value::Map(fields)) = almena_format::cbor::read(bytes) else {
            return Err(Unreadable);
        };
        match fields.get(&ASKING) {
            Some(&Value::Uint(SINCE)) => match fields.get(&FROM) {
                Some(&Value::Uint(from)) => Ok(Self::Since(from)),
                _ => Err(Unreadable),
            },
            Some(&Value::Uint(ONE)) => match fields.get(&NAMED) {
                Some(Value::Text(name)) => Name::parse(name).map(Self::Act).map_err(|_| Unreadable),
                _ => Err(Unreadable),
            },
            Some(&Value::Uint(ROOT)) => match fields.get(&EPOCH) {
                Some(&Value::Uint(epoch)) => Ok(Self::Root(epoch)),
                _ => Err(Unreadable),
            },
            Some(&Value::Uint(GROWN)) => match fields.get(&REACHED) {
                Some(&Value::Uint(written)) => Ok(Self::Grown(written)),
                _ => Err(Unreadable),
            },
            Some(&Value::Uint(SAW)) => {
                let (Some(&Value::Uint(epoch)), Some(Value::Bytes(signature))) =
                    (fields.get(&EPOCH), fields.get(&SIGNED))
                else {
                    return Err(Unreadable);
                };
                signature
                    .as_slice()
                    .try_into()
                    .map(|signature| Self::Saw(epoch, signature))
                    .map_err(|_| Unreadable)
            }
            _ => Err(Unreadable),
        }
    }
}

impl Said {
    /// The bytes of this answer.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let acts = self
            .acts
            .iter()
            .map(|act| Value::Bytes(act.clone()))
            .collect();
        let mut fields: std::collections::BTreeMap<u64, Value> = [
            (ACTS, Value::Array(acts)),
            (WRITTEN, Value::Uint(self.written)),
        ]
        .into_iter()
        .collect();
        if let Some(root) = &self.root {
            fields.insert(ROOT_SAID, Value::Bytes(root.clone()));
        }
        Value::Map(fields).to_bytes()
    }

    /// An answer read back off the wire.
    ///
    /// # Errors
    ///
    /// [`Unreadable`].
    pub fn read(bytes: &[u8]) -> Result<Self, Unreadable> {
        let Ok(Value::Map(fields)) = almena_format::cbor::read(bytes) else {
            return Err(Unreadable);
        };
        let (Some(Value::Array(acts)), Some(&Value::Uint(written))) =
            (fields.get(&ACTS), fields.get(&WRITTEN))
        else {
            return Err(Unreadable);
        };

        let mut out = Vec::with_capacity(acts.len());
        for act in acts {
            match act {
                Value::Bytes(bytes) => out.push(bytes.clone()),
                // One unreadable act spoils the answer rather than being skipped: the acts arrive
                // in order and are applied in order, and a hole in the middle of that is not a
                // shorter answer, it is a different one.
                _ => return Err(Unreadable),
            }
        }
        let root = match fields.get(&ROOT_SAID) {
            Some(Value::Bytes(bytes)) => Some(bytes.clone()),
            // Absent is *nothing to say about that epoch*. Anything else in its place is a message
            // this build cannot read, and reading it as absent would be inventing an answer.
            Some(_) => return Err(Unreadable),
            None => None,
        };
        Ok(Self {
            acts: out,
            written,
            root,
        })
    }
}

/// The largest message this will read off a wire.
///
/// A node asks for a page at a time, so nothing legitimate approaches this. It is here so that a
/// length arriving from somewhere else cannot ask for an allocation the size of the machine.
const LARGEST: u64 = 8 * 1024 * 1024;

/// Reading and writing the two messages, length first.
///
/// The length has to be on the wire because a stream has no edges: without it a reader cannot tell
/// where one message stops, and a message that ran into the next would be neither.
#[derive(Debug, Clone, Default)]
pub struct Talking;

#[async_trait::async_trait]
impl libp2p::request_response::Codec for Talking {
    type Protocol = String;
    type Request = Ask;
    type Response = Said;

    async fn read_request<T>(&mut self, _: &Self::Protocol, io: &mut T) -> std::io::Result<Ask>
    where
        T: futures::AsyncRead + Unpin + Send,
    {
        let bytes = framed(io).await?;
        Ask::read(&bytes).map_err(|_| std::io::Error::other("not a question this build reads"))
    }

    async fn read_response<T>(&mut self, _: &Self::Protocol, io: &mut T) -> std::io::Result<Said>
    where
        T: futures::AsyncRead + Unpin + Send,
    {
        let bytes = framed(io).await?;
        Said::read(&bytes).map_err(|_| std::io::Error::other("not an answer this build reads"))
    }

    async fn write_request<T>(
        &mut self,
        _: &Self::Protocol,
        io: &mut T,
        ask: Ask,
    ) -> std::io::Result<()>
    where
        T: futures::AsyncWrite + Unpin + Send,
    {
        frame(io, &ask.to_bytes()).await
    }

    async fn write_response<T>(
        &mut self,
        _: &Self::Protocol,
        io: &mut T,
        said: Said,
    ) -> std::io::Result<()>
    where
        T: futures::AsyncWrite + Unpin + Send,
    {
        frame(io, &said.to_bytes()).await
    }
}

/// One message off a stream, its length first.
async fn framed<T>(io: &mut T) -> std::io::Result<Vec<u8>>
where
    T: futures::AsyncRead + Unpin + Send,
{
    use futures::AsyncReadExt as _;

    let mut length = [0u8; 4];
    io.read_exact(&mut length).await?;
    let length = u64::from(u32::from_be_bytes(length));
    if length > LARGEST {
        return Err(std::io::Error::other("longer than anything this answers"));
    }

    let mut bytes = vec![0u8; length as usize];
    io.read_exact(&mut bytes).await?;
    Ok(bytes)
}

/// One message onto a stream, its length first.
async fn frame<T>(io: &mut T, bytes: &[u8]) -> std::io::Result<()>
where
    T: futures::AsyncWrite + Unpin + Send,
{
    use futures::AsyncWriteExt as _;

    let length = u32::try_from(bytes.len())
        .map_err(|_| std::io::Error::other("longer than anything this sends"))?;
    io.write_all(&length.to_be_bytes()).await?;
    io.write_all(bytes).await?;
    io.close().await
}

#[cfg(test)]
mod tests {
    use super::{Ask, Said, Unreadable};
    use almena_format::identifier::Name;

    fn a_name() -> Name {
        Name::of(b"something")
    }

    #[test]
    fn a_question_survives_the_wire() {
        for question in [
            Ask::Since(0),
            Ask::Since(9_000),
            Ask::Act(a_name()),
            Ask::Root(4),
            Ask::Grown(9_001),
            Ask::Saw(7, [3u8; 64]),
        ] {
            assert_eq!(
                Ask::read(&question.to_bytes()),
                Ok(question.clone()),
                "{question:?}"
            );
        }
    }

    #[test]
    fn an_answer_survives_the_wire_with_its_acts_untouched() {
        // The acts must come back byte for byte: the name of an act is the hash of its bytes, so
        // anything re-encoded on the way has been renamed.
        let said = Said {
            acts: vec![b"one act".to_vec(), b"another".to_vec()],
            written: 12,
            root: None,
        };
        assert_eq!(Said::read(&said.to_bytes()), Ok(said));
    }

    #[test]
    fn an_empty_answer_is_an_answer() {
        // What a node that is already up to date gets, which is the common case.
        let said = Said {
            acts: Vec::new(),
            written: 4,
            root: None,
        };
        let back = Said::read(&said.to_bytes()).expect("readable");
        assert!(back.acts.is_empty());
        assert_eq!(back.written, 4, "and it still says how far it has got");
    }

    #[test]
    fn a_root_survives_the_wire_beside_the_acts() {
        let said = Said {
            acts: Vec::new(),
            written: 9,
            root: Some(b"what a node signed about an epoch".to_vec()),
        };
        assert_eq!(Said::read(&said.to_bytes()), Ok(said));
    }

    #[test]
    fn an_answer_with_nothing_to_say_about_an_epoch_is_still_an_answer() {
        // A node that has not closed that epoch has nothing to say, and inventing something would
        // be worse than saying so.
        let said = Said {
            acts: Vec::new(),
            written: 9,
            root: None,
        };
        assert_eq!(Said::read(&said.to_bytes()).expect("readable").root, None);
    }

    #[test]
    fn a_question_this_build_does_not_know_is_refused_and_not_guessed_at() {
        // A third question would arrive with a number this build has never seen, and answering it
        // as though it were one of the two would be answering something nobody asked.
        let unknown = almena_format::cbor::Value::Map(
            [(1u64, almena_format::cbor::Value::Uint(99))]
                .into_iter()
                .collect(),
        );
        assert_eq!(Ask::read(&unknown.to_bytes()), Err(Unreadable));
    }

    #[test]
    fn a_question_missing_what_it_needs_is_refused() {
        let missing = almena_format::cbor::Value::Map(
            [(1u64, almena_format::cbor::Value::Uint(1))]
                .into_iter()
                .collect(),
        );
        assert_eq!(Ask::read(&missing.to_bytes()), Err(Unreadable));
    }

    #[test]
    fn bytes_that_are_not_canonical_are_not_read() {
        // The same rule as everywhere else here: canonicity is checked on the way in, so that two
        // encodings of one question cannot both be that question.
        assert_eq!(Ask::read(b"not cbor at all"), Err(Unreadable));
        assert_eq!(Said::read(&[]), Err(Unreadable));
    }
}
