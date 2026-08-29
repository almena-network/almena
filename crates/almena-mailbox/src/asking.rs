//! What a device says to its mediator, and how it proves it may.
//!
//! # Reading is not authenticated, and this is not reading
//!
//! Everything a node serves about the record is public, so nothing there has a caller. **A mailbox
//! is not part of the record.** It holds one person's post, and who may take it is exactly one
//! device — so this is the one surface of a node where the question *who is asking* has to be asked
//! at all, and it is asked the way everything else here is: by a signature over bytes, with no
//! session to be in and none to be thrown out of.
//!
//! Delivering is not here. A sender is a stranger with a relationship, and the relationship *is*
//! their authorisation (`SPECS.md §6.5`): they can write where they were given an address, and
//! nowhere else. Taking, confirming and declaring are the account's own, and those are these.
//!
//! # What proves it
//!
//! The key that signs must be a device the account's own chain says is authorised — which the node
//! already holds, because it replayed that chain like every other. So there is no second register
//! of who may collect, and no way for one to drift from the other: **taking a device off an account
//! takes it off the mailbox in the same act**, with nothing to remember to do afterwards.
//!
//! # Why the epoch is in what gets signed
//!
//! Otherwise one overheard asking is a key to that mailbox for ever. It is signed, so it cannot be
//! altered, and the mediator refuses anything dated outside a narrow window around now — which
//! costs a device nothing, since a device that cannot reach its mediator cannot collect anyway.

use almena_format::cbor::Value;
use almena_format::identifier::{Did, Name};
use almena_suite::p256;
use almena_time::{Epoch, Epochs};

/// How far from now an asking may be dated and still be answered.
///
/// **Two epochs either side.** Ahead as well as behind, because a device reads the epoch from a
/// node's own answer and two nodes a moment apart would otherwise make one of them wrong.
pub const WITHIN: Epochs = Epochs(2);

/// Where each part of an asking sits.
mod field {
    /// Which of the errands this is.
    pub const ERRAND: u64 = 1;
    /// Whose mailbox.
    pub const WHOSE: u64 = 3;
    /// Which device is asking, by the key it operates with.
    pub const DEVICE: u64 = 5;
    /// Which epoch it was written in.
    pub const AT: u64 = 7;
    /// What it names, which each errand reads its own way.
    pub const NAMES: u64 = 9;
    /// The signature over everything above.
    pub const SIGNED: u64 = 11;
}

/// What a device is asking its mediator to do.
///
/// **All three critical**, which is what the odd numbers say: an errand a build does not recognise
/// is one it must refuse rather than guess at, because guessing between *take* and *destroy* is the
/// guess with a mailbox on the other side of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Errand {
    /// Say which devices and relationships this account has.
    Carry = 1,
    /// Take what is waiting.
    Collect = 3,
    /// Say those messages arrived, so that they stop being held.
    Confirm = 5,
}

impl Errand {
    /// The errand that number is, if it is one this build knows.
    #[must_use]
    pub const fn of(number: u64) -> Option<Self> {
        match number {
            1 => Some(Self::Carry),
            3 => Some(Self::Collect),
            5 => Some(Self::Confirm),
            _ => None,
        }
    }
}

/// One asking, as it arrived.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asking {
    /// What it wants done.
    pub errand: Errand,
    /// Whose mailbox it is about.
    pub whose: Did,
    /// The device asking, by the key it operates with.
    pub device: Vec<u8>,
    /// The epoch it was written in.
    pub at: Epoch,
    /// What it names: the relationships for [`Errand::Carry`], the messages for [`Errand::Confirm`].
    pub names: Vec<String>,
    /// The signature over everything else.
    pub signed: Vec<u8>,
}

/// Why an asking was not carried out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Not {
    /// These bytes are not an asking, or name something this build does not know.
    Unreadable,
    /// Dated too far from now to be this device asking now.
    OutOfTime,
    /// The signature does not hold, or the key that made it is not on that account.
    NotThatDevice,
}

impl Asking {
    /// The bytes a device signs, which are everything it says except the signature.
    ///
    /// The account is in here and so is the device: a signature over the errand alone would be one
    /// a mediator could replay against a different mailbox of the same person's.
    #[must_use]
    pub fn over(&self) -> Vec<u8> {
        Value::Map(
            [
                (field::ERRAND, Value::Uint(self.errand as u64)),
                (field::WHOSE, Value::Text(self.whose.to_string())),
                (field::DEVICE, Value::Bytes(self.device.clone())),
                (field::AT, Value::Uint(self.at.number())),
                (
                    field::NAMES,
                    Value::Array(self.names.iter().cloned().map(Value::Text).collect()),
                ),
            ]
            .into_iter()
            .collect(),
        )
        .to_bytes()
    }

    /// Read one from the bytes a device sent.
    ///
    /// # Errors
    ///
    /// [`Not::Unreadable`] for anything that is not one of these.
    pub fn read(bytes: &[u8]) -> Result<Self, Not> {
        let Ok(Value::Map(fields)) = almena_format::cbor::read(bytes) else {
            return Err(Not::Unreadable);
        };
        let errand = match fields.get(&field::ERRAND) {
            Some(Value::Uint(number)) => Errand::of(*number).ok_or(Not::Unreadable)?,
            _ => return Err(Not::Unreadable),
        };
        let Some(Value::Text(whose)) = fields.get(&field::WHOSE) else {
            return Err(Not::Unreadable);
        };
        let whose = Did::parse(whose).map_err(|_| Not::Unreadable)?;
        let Some(Value::Bytes(device)) = fields.get(&field::DEVICE) else {
            return Err(Not::Unreadable);
        };
        let Some(Value::Uint(at)) = fields.get(&field::AT) else {
            return Err(Not::Unreadable);
        };
        let Some(Value::Array(named)) = fields.get(&field::NAMES) else {
            return Err(Not::Unreadable);
        };
        let mut names = Vec::with_capacity(named.len());
        for one in named {
            let Value::Text(text) = one else {
                return Err(Not::Unreadable);
            };
            names.push(text.clone());
        }
        let Some(Value::Bytes(signed)) = fields.get(&field::SIGNED) else {
            return Err(Not::Unreadable);
        };
        Ok(Self {
            errand,
            whose,
            device: device.clone(),
            at: Epoch::new(*at),
            names,
            signed: signed.clone(),
        })
    }

    /// Everything it says, as bytes, ready to be sent.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let Ok(Value::Map(mut fields)) = almena_format::cbor::read(&self.over()) else {
            // `over` built it, so it reads. Nothing sensible to do with an impossible branch except
            // hand back something that will not be mistaken for an asking.
            return Vec::new();
        };
        fields.insert(field::SIGNED, Value::Bytes(self.signed.clone()));
        Value::Map(fields).to_bytes()
    }

    /// Sign one, which is what a device does before sending it.
    #[must_use]
    pub fn signed_by(mut self, key: &p256::SigningKey) -> Self {
        // The device goes in **before** the signature is made, and not after. Signed the other way
        // round, the field naming who is asking would be the one field nobody signed — which is
        // exactly the field a signature over the errand alone leaves free to change.
        self.device = key.verifying_key().bytes().to_vec();
        self.signed = key.sign(&self.over()).bytes().to_vec();
        self
    }

    /// Whether this device may ask this, given what the account's chain says and what time it is.
    ///
    /// `devices` is the account's own list, as the node replayed it. **Nothing else is consulted**,
    /// which is what keeps removing a device from needing a second thing done afterwards.
    ///
    /// # Errors
    ///
    /// [`Not::OutOfTime`] for something dated outside the window; [`Not::NotThatDevice`] for a
    /// signature that does not hold or a key the account does not authorise.
    pub fn holds(&self, devices: &[Vec<u8>], now: Epoch) -> Result<(), Not> {
        let apart = now.number().abs_diff(self.at.number());
        if apart > WITHIN.count() {
            return Err(Not::OutOfTime);
        }
        if !devices.iter().any(|key| key == &self.device) {
            return Err(Not::NotThatDevice);
        }
        let key: [u8; p256::PUBLIC_KEY_WIDTH] = self
            .device
            .as_slice()
            .try_into()
            .map_err(|_| Not::NotThatDevice)?;
        let signature: [u8; p256::SIGNATURE_WIDTH] = self
            .signed
            .as_slice()
            .try_into()
            .map_err(|_| Not::NotThatDevice)?;
        let key = p256::VerifyingKey::from_bytes(key).map_err(|_| Not::NotThatDevice)?;
        let signature = p256::Signature::from_bytes(signature).map_err(|_| Not::NotThatDevice)?;
        key.verify(&self.over(), &signature)
            .map_err(|_| Not::NotThatDevice)
    }

    /// What it names, read as message names, for [`Errand::Confirm`].
    ///
    /// Anything in the list that is not a name is dropped rather than refused: a confirmation names
    /// what arrived, and one unreadable entry is no reason to keep holding the rest.
    #[must_use]
    pub fn named(&self) -> Vec<Name> {
        self.names
            .iter()
            .filter_map(|text| Name::parse(text).ok())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::{Asking, Errand, Not, WITHIN};
    use almena_format::identifier::{Did, Name, Network};
    use almena_suite::p256;
    use almena_time::Epoch;

    fn key(seed: u8) -> p256::SigningKey {
        p256::SigningKey::from_secret([seed.max(1); 32]).expect("a key")
    }

    fn whose() -> Did {
        Did::new(Network::Development, Name::of(b"an account"))
    }

    fn asking(errand: Errand, at: Epoch) -> Asking {
        Asking {
            errand,
            whose: whose(),
            device: Vec::new(),
            at,
            names: Vec::new(),
            signed: Vec::new(),
        }
    }

    #[test]
    fn what_a_device_signs_survives_the_round_trip() {
        let signed = asking(Errand::Collect, Epoch::new(9)).signed_by(&key(1));
        let read = Asking::read(&signed.to_bytes()).expect("readable");
        assert_eq!(read, signed);
        read.holds(&[key(1).verifying_key().bytes().to_vec()], Epoch::new(9))
            .expect("its own device, at its own epoch");
    }

    #[test]
    fn a_key_the_account_does_not_authorise_collects_nothing() {
        // **The account's own chain is the only register.** A device taken off it stops being able
        // to collect in that same act, with nothing to remember to do here afterwards.
        let signed = asking(Errand::Collect, Epoch::new(9)).signed_by(&key(1));
        assert_eq!(
            signed.holds(&[key(2).verifying_key().bytes().to_vec()], Epoch::new(9)),
            Err(Not::NotThatDevice)
        );
        assert_eq!(signed.holds(&[], Epoch::new(9)), Err(Not::NotThatDevice));
    }

    #[test]
    fn one_overheard_asking_is_not_a_key_to_that_mailbox_for_ever() {
        let signed = asking(Errand::Collect, Epoch::new(9)).signed_by(&key(1));
        let devices = [key(1).verifying_key().bytes().to_vec()];
        assert_eq!(
            signed.holds(&devices, Epoch::new(9 + WITHIN.count() + 1)),
            Err(Not::OutOfTime)
        );
        // And ahead as well as behind: two nodes a moment apart must not make one device wrong.
        signed
            .holds(&devices, Epoch::new(9 - WITHIN.count()))
            .expect("within, on the early side");
    }

    #[test]
    fn changing_a_word_of_it_breaks_the_signature() {
        let mut signed = asking(Errand::Collect, Epoch::new(9)).signed_by(&key(1));
        let devices = [key(1).verifying_key().bytes().to_vec()];
        signed.errand = Errand::Confirm;
        assert_eq!(
            signed.holds(&devices, Epoch::new(9)),
            Err(Not::NotThatDevice)
        );

        // And so does changing whose mailbox it is about, which is the field that would otherwise
        // let one asking be replayed against a different mailbox of the same person's.
        let mut elsewhere = asking(Errand::Collect, Epoch::new(9)).signed_by(&key(1));
        elsewhere.whose = Did::new(Network::Development, Name::of(b"somebody else"));
        assert_eq!(
            elsewhere.holds(&devices, Epoch::new(9)),
            Err(Not::NotThatDevice)
        );
    }

    #[test]
    fn an_errand_this_build_does_not_know_is_refused_and_not_guessed_at() {
        // Guessing between *take* and *destroy* is the guess with somebody's post on the other side.
        assert!(Errand::of(7).is_none());
        let mut signed = asking(Errand::Collect, Epoch::new(9)).signed_by(&key(1));
        signed.names = vec!["not a name".to_owned()];
        assert!(
            signed.named().is_empty(),
            "and unreadable names are dropped"
        );
    }
}
