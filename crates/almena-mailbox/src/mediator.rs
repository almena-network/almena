//! Every account this mediator carries post for.
//!
//! **A capability and not a duty** (`SPECS.md §4.1`): carrying messages for people whose devices
//! are off is one of the things a node can be switched on to do, chosen by whoever runs it. A node
//! with it off has none of this and says so where capabilities are counted.
//!
//! # Who gets a mailbox
//!
//! Whoever asks. A mediator holds post for accounts that name it, and naming it is what a person's
//! own record says — so this does not decide who may have one, it decides what happens to what
//! arrives. What stops a mediator being filled by accounts that do not exist is that a mailbox
//! nobody comes to stops being one within ninety days, and that every delivery is attributable
//! (`SPECS.md §6.4`): whoever may write is an object of the record with a public identity.

use std::collections::BTreeMap;

use almena_format::identifier::{Did, Name};
use almena_time::Epoch;

use crate::account::{Account, Refused, TurnedAway};
use crate::held::Held;

/// The post one node is holding, for everybody who asked it to.
#[derive(Debug, Default, Clone)]
pub struct Mediator {
    /// By the account it belongs to, which is what the account-wide ceiling is counted over.
    accounts: BTreeMap<Did, Account>,
}

impl Mediator {
    /// A mediator holding nothing.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Hold post for that account, with a mailbox for each of those devices.
    ///
    /// **Asked for again is not asked for twice.** A device added or removed changes the set, and
    /// what already waits for a device that is still there stays where it is — losing somebody's
    /// post because they added a laptop would be the wrong answer to a routine act.
    pub fn carry(
        &mut self,
        whose: &Did,
        devices: impl IntoIterator<Item = Vec<u8>>,
        relations: impl IntoIterator<Item = String>,
        at: Epoch,
    ) {
        let devices: Vec<Vec<u8>> = devices.into_iter().collect();
        let account = match self.accounts.get_mut(whose) {
            Some(held) => {
                held.devices(devices);
                held
            }
            None => self
                .accounts
                .entry(whose.clone())
                .or_insert_with(|| Account::of(devices, at)),
        };
        account.relates(relations);
    }

    /// Whether this mediator carries post for that account.
    #[must_use]
    pub fn carries(&self, whose: &Did) -> bool {
        self.accounts.contains_key(whose)
    }

    /// Whether that is a relationship the account declared.
    ///
    /// **What decides between a mailbox and the doorbell**, and it is the account's answer rather
    /// than the sender's: somebody writing under a name nobody has used is somebody with no
    /// relationship, whatever they say they are.
    #[must_use]
    pub fn knows(&self, whose: &Did, relation: &str) -> bool {
        self.accounts
            .get(whose)
            .is_some_and(|account| account.knows(relation))
    }

    /// Whose mailbox an address belongs to.
    ///
    /// **A sender addresses a relationship, not an account** (`SPECS.md §6.5`: *the mediator already
    /// routes by peer identifier*). That is the whole of why a relationship can exist without either
    /// end learning the other's root identifier: what the sender was given is an address, and this
    /// is the only party that has to know which of its customers answers to it.
    ///
    /// A **root identifier** resolves too, and to the same account — that is the doorbell, which
    /// `SPECS.md §6.5` sends to the root and which is the one thing that reaches somebody with no
    /// relationship yet.
    #[must_use]
    pub fn addressed(&self, to: &str) -> Option<&Did> {
        if let Ok(whose) = Did::parse(to)
            && let Some((held, _)) = self.accounts.get_key_value(&whose)
        {
            return Some(held);
        }
        self.accounts
            .iter()
            .find(|(_, account)| account.knows(to))
            .map(|(whose, _)| whose)
    }

    /// Take a message into one device's mailbox.
    ///
    /// # Errors
    ///
    /// [`Refused`], which is what the sender is told — and what the recipient is counted, so that
    /// silence and a blocked mailbox are two different things to them (`SPECS.md §6.5`).
    pub fn deliver(&mut self, whose: &Did, message: &Held, at: Epoch) -> Result<(), Refused> {
        let account = self.accounts.get_mut(whose).ok_or(Refused::NoSuchMailbox)?;

        // **Into every mailbox, or into none.** A sender delivers to every mailbox the recipient
        // declared (`SPECS.md §6.2`), so that deletion after collection works per mailbox without
        // two devices racing. All or nothing, because a message that reached the laptop and not the
        // phone is one the two devices disagree about — and disagreeing about what arrived is worse
        // than not having it.
        let devices = account.devices_held();
        for to in &devices {
            account.room_for_one(to, message, at)?;
        }
        for to in &devices {
            account.take(to, message.clone());
        }
        Ok(())
    }

    /// Take a message addressed to the root identifier rather than to a relationship.
    ///
    /// # Errors
    ///
    /// [`Refused`], as [`Mediator::deliver`].
    pub fn ring(&mut self, whose: &Did, message: Held, at: Epoch) -> Result<(), Refused> {
        self.accounts
            .get_mut(whose)
            .ok_or(Refused::NoSuchMailbox)?
            .ring(message, at)
    }

    /// What is waiting for that device, oldest first, and what has been turned away.
    ///
    /// **Coming for the post is what keeps a mailbox a mailbox**, so asking counts as collecting:
    /// a device that reads its post every week is a device this mediator goes on holding for.
    pub fn collect(&mut self, whose: &Did, to: &[u8], at: Epoch) -> Option<Collection> {
        let account = self.accounts.get_mut(whose)?;
        if account.inactive(at) {
            // Gone, and said. The contents are dropped and the account starts again from now, so
            // that somebody who comes back finds an empty mailbox rather than nothing at all.
            account.emptied(at);
            return Some(Collection {
                waiting: Vec::new(),
                ringing: Vec::new(),
                turned_away: None,
                was_inactive: true,
            });
        }
        account.collected(at);
        Some(Collection {
            waiting: account.waiting(to).unwrap_or_default().to_vec(),
            ringing: account.ringing().to_vec(),
            turned_away: account.turned_away(),
            was_inactive: false,
        })
    }

    /// Say where to wake one of an account's devices.
    ///
    /// **Given here and nowhere else** (`SPECS.md §6.3`). It does not go in the root identifier,
    /// which is public and enumerable — waking somebody's telephone is exactly the abuse that
    /// section is defended against, and the two could not both be true.
    ///
    /// False for an account this mediator does not carry, or a device it holds no mailbox for.
    pub fn wakes_at(&mut self, whose: &Did, device: &[u8], endpoint: &str) -> bool {
        self.accounts
            .get_mut(whose)
            .is_some_and(|account| account.wakes_at(device, endpoint))
    }

    /// Where to wake that device, if it has said.
    ///
    /// **What the mediator holds is somewhere to deliver a signal to**, and nothing about how it
    /// gets there: that is what keeps the notification path from becoming a dependency.
    #[must_use]
    pub fn wake(&self, whose: &Did, device: &[u8]) -> Option<&str> {
        self.accounts
            .get(whose)
            .and_then(|account| account.wake(device))
    }

    /// Say those messages were collected, so that they stop being held.
    pub fn confirm(&mut self, whose: &Did, to: &[u8], names: &[Name], at: Epoch) {
        if let Some(account) = self.accounts.get_mut(whose) {
            account.confirm(to, names, at);
        }
    }
}

/// What one collection hands over.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Collection {
    /// What is waiting in that device's mailbox, oldest first.
    pub waiting: Vec<Held>,
    /// What is waiting at the doorbell, which belongs to the account and not to a device.
    pub ringing: Vec<Held>,
    /// What was turned away in this account's name, and since when.
    pub turned_away: Option<TurnedAway>,
    /// Whether this mailbox had gone inactive, which is what the app tells its owner about.
    ///
    /// **Said once, on the first collection after it happened.** A mediator has nobody to tell at
    /// the moment it happens — that is what inactive means — so the telling waits for whoever
    /// comes back (`SPECS.md §6.2`).
    pub was_inactive: bool,
}

#[cfg(test)]
mod tests {
    use super::Mediator;
    use crate::account::Refused;
    use crate::held::Held;
    use crate::quota;
    use almena_format::identifier::{Did, Name, Network};
    use almena_time::{Epoch, Epochs};

    fn relations() -> Vec<String> {
        ["zIssuer".to_owned(), "zStranger".to_owned()].to_vec()
    }

    fn whose() -> Did {
        Did::new(Network::Development, Name::of(b"an account"))
    }

    fn device(mark: u8) -> Vec<u8> {
        let mut key = vec![0x02];
        key.extend_from_slice(&[mark; 32]);
        key
    }

    fn message(mark: u8, relation: &str, size: usize) -> Held {
        // The mark goes *into* the bytes, because the name is their hash: two messages this
        // helper builds are two messages exactly when it was asked for two.
        Held::new(
            relation.to_owned(),
            vec![mark; size],
            Epochs(24),
            Epoch::GENESIS,
        )
    }

    #[test]
    fn a_sender_delivers_into_the_mailbox_of_a_device_that_is_off_and_the_client_finds_it() {
        // **The exit criterion's first clause.** Nothing here needs the recipient to be present:
        // that is the whole of what a mediator is for.
        let mut mediator = Mediator::new();
        mediator.carry(
            &whose(),
            [device(1)],
            ["zIssuer".to_owned(), "zStranger".to_owned()],
            Epoch::GENESIS,
        );

        mediator
            .deliver(&whose(), &message(1, "zIssuer", 400), Epoch::GENESIS)
            .expect("room");

        let collected = mediator
            .collect(&whose(), &device(1), Epoch::new(2))
            .expect("a mailbox");
        assert_eq!(collected.waiting.len(), 1);
        assert!(!collected.was_inactive);

        // And confirming is what makes it go, so a dropped connection does not cost a message.
        let called = collected.waiting[0].called.clone();
        mediator.confirm(&whose(), &device(1), &[called], Epoch::new(2));
        assert!(
            mediator
                .collect(&whose(), &device(1), Epoch::new(3))
                .expect("a mailbox")
                .waiting
                .is_empty()
        );
    }

    #[test]
    fn adding_a_device_does_not_cost_what_is_waiting_for_the_other() {
        let mut mediator = Mediator::new();
        mediator.carry(
            &whose(),
            [device(1)],
            ["zIssuer".to_owned(), "zStranger".to_owned()],
            Epoch::GENESIS,
        );
        mediator
            .deliver(&whose(), &message(1, "zIssuer", 400), Epoch::GENESIS)
            .expect("room");

        mediator.carry(
            &whose(),
            [device(1), device(2)],
            ["zIssuer".to_owned(), "zStranger".to_owned()],
            Epoch::GENESIS,
        );
        assert_eq!(
            mediator
                .collect(&whose(), &device(1), Epoch::GENESIS)
                .expect("a mailbox")
                .waiting
                .len(),
            1,
            "the post is where it was"
        );
    }

    #[test]
    fn a_delivery_reaches_every_mailbox_or_none_of_them() {
        // **A sender delivers to every mailbox the recipient declared** (`SPECS.md §6.2`), and if
        // one of them has no room the delivery does not happen at all. Two devices disagreeing
        // about what arrived is worse than neither having it: the phone would show an offer the
        // laptop says was never made, and there is no way for either of them to find out which is
        // right.
        let mut mediator = Mediator::new();
        mediator.carry(&whose(), [device(1)], relations(), Epoch::GENESIS);

        // Fill one relation to its ceiling, on the only device there is.
        let big = u8::try_from(quota::RELATION_MOST / quota::MESSAGE_MOST).expect("a few");
        for which in 0..big {
            mediator
                .deliver(
                    &whose(),
                    &message(which, "zIssuer", quota::MESSAGE_MOST),
                    Epoch::GENESIS,
                )
                .expect("room while there is room");
        }

        // Now there is a second device, whose own mailbox is empty.
        mediator.carry(
            &whose(),
            [device(1), device(2)],
            relations(),
            Epoch::GENESIS,
        );
        assert_eq!(
            mediator.deliver(&whose(), &message(big, "zIssuer", 400), Epoch::GENESIS),
            Err(Refused::RelationFull),
            "the full mailbox decides for all of them"
        );
        assert!(
            mediator
                .collect(&whose(), &device(2), Epoch::GENESIS)
                .expect("a mailbox")
                .waiting
                .is_empty(),
            "and the empty one is left empty rather than half-told"
        );

        room_again_and_every_mailbox_gets_it(&mut mediator, big);
    }

    /// The second half of the test above, which is one function only because of its length.
    fn room_again_and_every_mailbox_gets_it(mediator: &mut Mediator, big: u8) {
        mediator.confirm(
            &whose(),
            &device(1),
            &[message(0, "zIssuer", quota::MESSAGE_MOST).called],
            Epoch::GENESIS,
        );
        mediator
            .deliver(&whose(), &message(big, "zIssuer", 400), Epoch::GENESIS)
            .expect("room now");
        for which in [1u8, 2] {
            assert_eq!(
                mediator
                    .collect(&whose(), &device(which), Epoch::GENESIS)
                    .expect("a mailbox")
                    .waiting
                    .last()
                    .map(|held| held.called.clone()),
                Some(message(big, "zIssuer", 400).called),
                "device {which} has it"
            );
        }
    }

    #[test]
    fn an_account_this_mediator_does_not_carry_for_is_said_and_not_invented() {
        let mut mediator = Mediator::new();
        assert_eq!(
            mediator.deliver(&whose(), &message(1, "z", 10), Epoch::GENESIS),
            Err(Refused::NoSuchMailbox)
        );
        assert!(
            mediator
                .collect(&whose(), &device(1), Epoch::GENESIS)
                .is_none()
        );
    }

    #[test]
    fn a_mailbox_that_went_inactive_says_so_once_to_whoever_comes_back() {
        // A mediator has nobody to tell at the moment it happens — that is what inactive means —
        // so the telling waits for whoever comes back (`SPECS.md §6.2`).
        let mut mediator = Mediator::new();
        mediator.carry(
            &whose(),
            [device(1)],
            ["zIssuer".to_owned(), "zStranger".to_owned()],
            Epoch::GENESIS,
        );
        mediator
            .deliver(&whose(), &message(1, "zIssuer", 400), Epoch::GENESIS)
            .expect("room");

        let long_after = Epoch::new(quota::UNCOLLECTED_UNTIL_INACTIVE.now());
        let collected = mediator
            .collect(&whose(), &device(1), long_after)
            .expect("a mailbox");
        assert!(collected.was_inactive, "said");
        assert!(collected.waiting.is_empty(), "and the contents are gone");

        // And it is a mailbox again from now, rather than nothing at all.
        mediator
            .deliver(&whose(), &message(2, "zIssuer", 400), long_after)
            .expect("carrying again");
        assert!(
            !mediator
                .collect(&whose(), &device(1), long_after)
                .expect("a mailbox")
                .was_inactive,
            "and said only once"
        );
    }

    #[test]
    fn the_doorbell_belongs_to_the_account_and_reaches_every_device() {
        // It is addressed to the root identifier, not to a device — so whichever device comes for
        // the post finds what was left there, and a person with two of them does not miss an
        // introduction because they picked up the wrong one.
        let mut mediator = Mediator::new();
        mediator.carry(
            &whose(),
            [device(1), device(2)],
            ["zIssuer".to_owned(), "zStranger".to_owned()],
            Epoch::GENESIS,
        );
        mediator
            .ring(&whose(), message(5, "zStranger", 300), Epoch::GENESIS)
            .expect("room");

        for key in [device(1), device(2)] {
            assert_eq!(
                mediator
                    .collect(&whose(), &key, Epoch::GENESIS)
                    .expect("a mailbox")
                    .ringing
                    .len(),
                1
            );
        }
    }
}
