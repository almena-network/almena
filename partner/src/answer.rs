//! What a node answers, read out of the bytes it sent.
//!
//! One map with five numbered fields, the same for every question: the epoch and the root it was
//! answered at, what happened as a number from one closed vocabulary, what was asked for when
//! there is anything, and which rule when the state is one that has rules. Written here a second
//! time rather than taken from `almena-api`, for the reason the holder's app writes it too: this
//! side of the wire is the side that has to be able to read a node without being the node.

use std::collections::BTreeMap;

use almena_format::cbor::Value;

use crate::failed::Failed;

/// Where each part of an answer sits.
mod field {
    /// The epoch it was answered in.
    pub const EPOCH: u64 = 1;
    /// The root over everything the node had written down.
    pub const ROOT: u64 = 2;
    /// What happened.
    pub const STATE: u64 = 3;
    /// What was asked for, when there is anything.
    pub const PAYLOAD: u64 = 4;
    /// Which rule, when the state is one that has rules.
    pub const WHICH: u64 = 5;
}

/// What a node says happened, as the closed vocabulary it comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// Here it is.
    Here,
    /// No creation with that name has been seen.
    DoesNotExist,
    /// It exists and this node will not say what it is.
    CannotResolve,
    /// It exists and is held elsewhere.
    NotHere,
    /// There is nothing at that path.
    NoSuchQuestion,
    /// The request could not be read.
    Malformed,
    /// A limit was reached.
    Throttled,
    /// Handed over and not taken; `which` names the rule.
    NotTaken,
    /// Handed over and written down.
    Taken,
    /// The question cannot be asked of this build yet.
    NotYetAskable,
    /// A number this build has no word for.
    Unknown(u64),
}

impl State {
    /// The state a number names.
    #[must_use]
    pub const fn of(number: u64) -> Self {
        match number {
            1 => Self::Here,
            2 => Self::DoesNotExist,
            3 => Self::CannotResolve,
            4 => Self::NotHere,
            5 => Self::NoSuchQuestion,
            6 => Self::Malformed,
            7 => Self::Throttled,
            8 => Self::NotTaken,
            9 => Self::Taken,
            10 => Self::NotYetAskable,
            other => Self::Unknown(other),
        }
    }

    /// The word this program writes for it.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Here => "here",
            Self::DoesNotExist => "does_not_exist",
            Self::CannotResolve => "cannot_resolve",
            Self::NotHere => "not_here",
            Self::NoSuchQuestion => "no_such_question",
            Self::Malformed => "malformed",
            Self::Throttled => "throttled",
            Self::NotTaken => "not_taken",
            Self::Taken => "taken",
            Self::NotYetAskable => "not_yet_askable",
            Self::Unknown(_) => "unknown",
        }
    }
}

/// One answer, as the node stamped it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
    /// The epoch the node answered in, which is the clock this program keeps time by.
    pub epoch: u64,
    /// The root the node had at that moment.
    pub root: Vec<u8>,
    /// What happened.
    pub state: State,
    /// What was asked for, when anything came with the answer.
    pub payload: Option<Value>,
    /// Which rule, when the state names one.
    pub which: Option<u64>,
}

impl Answer {
    /// One answer, read out of the bytes.
    ///
    /// # Errors
    ///
    /// `node_not_a_node` when the bytes are not the map every answer is.
    pub fn read(bytes: &[u8]) -> Result<Self, Failed> {
        let Ok(Value::Map(fields)) = almena_format::cbor::read(bytes) else {
            return Err(Failed::new("node_not_a_node"));
        };
        Self::from_fields(&fields).ok_or_else(|| Failed::new("node_not_a_node"))
    }

    fn from_fields(fields: &BTreeMap<u64, Value>) -> Option<Self> {
        let Some(Value::Uint(epoch)) = fields.get(&field::EPOCH) else {
            return None;
        };
        let Some(Value::Bytes(root)) = fields.get(&field::ROOT) else {
            return None;
        };
        let Some(Value::Uint(state)) = fields.get(&field::STATE) else {
            return None;
        };
        let which = match fields.get(&field::WHICH) {
            Some(Value::Uint(which)) => Some(*which),
            _ => None,
        };
        Some(Self {
            epoch: *epoch,
            root: root.clone(),
            state: State::of(*state),
            payload: fields.get(&field::PAYLOAD).cloned(),
            which,
        })
    }

    /// The answer's payload as text, where it is one.
    #[must_use]
    pub fn text(&self) -> Option<&str> {
        match &self.payload {
            Some(Value::Text(text)) => Some(text),
            _ => None,
        }
    }

    /// The answer's payload as bytes, where it is some.
    #[must_use]
    pub fn bytes(&self) -> Option<&[u8]> {
        match &self.payload {
            Some(Value::Bytes(bytes)) => Some(bytes),
            _ => None,
        }
    }

    /// The answer's payload as a map, where it is one.
    #[must_use]
    pub fn map(&self) -> Option<&BTreeMap<u64, Value>> {
        match &self.payload {
            Some(Value::Map(map)) => Some(map),
            _ => None,
        }
    }

    /// A failure naming the state, for an answer that was not the one wanted.
    #[must_use]
    pub fn refused(&self, word: &str) -> Failed {
        match self.which {
            Some(which) => {
                Failed::line(format!("{word} state={} which={which}", self.state.word()))
            }
            None => Failed::with(word, "state", self.state.word()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Answer, State};
    use almena_format::cbor::Value;
    use std::collections::BTreeMap;

    fn written(state: u64, payload: Option<Value>, which: Option<u64>) -> Vec<u8> {
        let mut fields = BTreeMap::from([
            (1, Value::Uint(42)),
            (2, Value::Bytes(vec![7; 32])),
            (3, Value::Uint(state)),
        ]);
        if let Some(payload) = payload {
            fields.insert(4, payload);
        }
        if let Some(which) = which {
            fields.insert(5, Value::Uint(which));
        }
        Value::Map(fields).to_bytes()
    }

    #[test]
    fn an_answer_is_read_with_its_stamp_and_its_state() {
        let answer = Answer::read(&written(1, Some(Value::Text("zHead".to_owned())), None))
            .expect("an answer");
        assert_eq!(answer.epoch, 42);
        assert_eq!(answer.state, State::Here);
        assert_eq!(answer.text(), Some("zHead"));
        assert_eq!(answer.which, None);
    }

    #[test]
    fn a_refusal_says_the_state_and_the_rule() {
        let answer = Answer::read(&written(8, None, Some(6))).expect("an answer");
        assert_eq!(answer.state, State::NotTaken);
        assert_eq!(
            answer.refused("act_not_taken").to_string(),
            "act_not_taken state=not_taken which=6"
        );
    }

    #[test]
    fn bytes_that_are_not_an_answer_are_said_to_be_no_node() {
        assert_eq!(
            Answer::read(b"not cbor").unwrap_err().to_string(),
            "node_not_a_node"
        );
        assert_eq!(
            Answer::read(&Value::Map(BTreeMap::new()).to_bytes())
                .unwrap_err()
                .to_string(),
            "node_not_a_node"
        );
    }

    #[test]
    fn a_number_this_build_has_no_word_for_is_kept_as_a_number() {
        assert_eq!(State::of(99), State::Unknown(99));
        assert_eq!(State::of(9), State::Taken);
    }
}
