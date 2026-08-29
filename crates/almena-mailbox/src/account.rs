//! The mailboxes of one person's devices, and the doorbell beside them.
//!
//! **One mailbox per device** (`SPECS.md §6.2`), because deletion after collection has to work per
//! mailbox: with one shared between a phone and a laptop, two devices collecting at once would race
//! over what has been delivered. A sender delivers into every mailbox the recipient has declared.
//!
//! **And a doorbell that is not a mailbox** (`SPECS.md §6.5`). The root identifier is public and
//! enumerable, so cold contact needs a channel of its own with a quota of its own — otherwise the
//! census is a list of addressable inboxes, and filling it silences every relationship a person
//! has. Separated, filling the doorbell costs them introductions and nothing else.
//!
//! > And recovery travels by it, so a blocked doorbell is not only *I cannot meet anybody today*:
//! > a recovery request that does not arrive is a recovery that does not happen. What bounds that
//! > is `SPECS.md §11.4`'s private guardian list — an attacker has to guess who they are — and the
//! > doorbell's own quota, which is here.

use std::collections::{BTreeMap, BTreeSet};

use almena_format::identifier::Name;
use almena_time::Epoch;

use crate::held::Held;
use crate::quota;

/// Why a delivery was not taken.
///
/// **Told to the sender, and counted for the recipient.** A refusal the recipient never hears about
/// is what makes filling somebody's mailbox an invisible attack (`SPECS.md §6.5`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    /// Larger than any one message may be.
    TooLarge,
    /// This relationship is holding as much as it may.
    RelationFull,
    /// The account is holding as much as it may, across every mailbox its devices have.
    AccountFull,
    /// The doorbell is holding as much as it may.
    DoorbellFull,
    /// There is no mailbox here by that name.
    NoSuchMailbox,
    /// Nothing addressed there is a relationship this account has.
    ///
    /// **Which is not a refusal to carry the message** — it is a refusal to carry it *here*. A
    /// sender with no relationship has the doorbell, and that is what it is for (`SPECS.md §6.5`).
    NoSuchRelation,
    /// Nobody has collected for long enough that this is not a mailbox any more.
    Inactive,
}

/// What one device's mailbox is holding.
#[derive(Debug, Default, Clone)]
pub struct Mailbox {
    /// What is in it, oldest first — which is the order it is handed over in.
    held: Vec<Held>,
}

impl Mailbox {
    /// What it is holding, oldest first.
    #[must_use]
    pub fn waiting(&self) -> &[Held] {
        &self.held
    }

    /// What one relationship is holding in it.
    fn from(&self, relation: &str) -> usize {
        self.held
            .iter()
            .filter(|one| one.relation == relation)
            .map(Held::weighs)
            .sum()
    }
}

/// Every mailbox of one account, its doorbell, and what has been turned away.
#[derive(Debug, Clone)]
pub struct Account {
    /// One per device, by the key that device operates with.
    mailboxes: BTreeMap<Vec<u8>, Mailbox>,
    /// The relationships this account has, which are the only ones with a floor of their own.
    ///
    /// **Declared, because a floor anybody can claim is not a floor.** The reserve exists so that
    /// one counterparty flooding cannot silence another; if a stranger could take one by writing
    /// under a name nobody has used, then inventing names would be a way to spend an account's
    /// whole ceiling from outside — and the ceiling would mean nothing.
    ///
    /// A relationship is a peer identifier, which `SPECS.md §3.3` makes unpublished and
    /// unenumerable: only its two ends know it. So the account tells its mediator the ones it has,
    /// which the mediator needs anyway to route by, and anything addressed elsewhere is somebody
    /// with no relationship — which is what the doorbell is for (`SPECS.md §6.5`).
    relations: BTreeSet<String>,
    /// What has arrived addressed to the root identifier rather than to a relationship.
    doorbell: Vec<Held>,
    /// When somebody last came for the post.
    collected: Epoch,
    /// Since when it has been turning deliveries away, and how many.
    turned_away: Option<TurnedAway>,
}

/// What the recipient is owed about deliveries made in their name and refused.
///
/// **Not a courtesy** (`SPECS.md §6.5`). Today a refusal goes to the sender alone, so the person
/// being attacked sees silence and cannot tell it from nobody writing. Saying *since this moment,
/// this many* is what turns an invisible attack into one they can act on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TurnedAway {
    /// When it started.
    pub since: Epoch,
    /// How many, since then.
    pub count: u64,
}

impl Account {
    /// An account with mailboxes for those devices and nothing in them.
    #[must_use]
    pub fn of(devices: impl IntoIterator<Item = Vec<u8>>, at: Epoch) -> Self {
        Self {
            mailboxes: devices
                .into_iter()
                .map(|key| (key, Mailbox::default()))
                .collect(),
            relations: BTreeSet::new(),
            doorbell: Vec::new(),
            collected: at,
            turned_away: None,
        }
    }

    /// Empty every mailbox, keeping the devices and the relationships.
    ///
    /// **What an account that went quiet for too long comes back to.** The post is dropped, because
    /// holding it for ever is the thing a mailbox cannot do; the account itself is not, because
    /// somebody returning to a phone they left in a drawer should find an empty mailbox and not a
    /// mediator that has forgotten them — and should not have to declare their relationships again
    /// to receive anything.
    pub fn emptied(&mut self, at: Epoch) {
        for mailbox in self.mailboxes.values_mut() {
            mailbox.held.clear();
        }
        self.doorbell.clear();
        self.turned_away = None;
        self.collected = at;
    }

    /// Say which relationships this account has.
    ///
    /// Each of them has a floor of its own; anything addressed elsewhere is a stranger, and reaches
    /// the doorbell rather than a mailbox.
    pub fn relates(&mut self, relations: impl IntoIterator<Item = String>) {
        self.relations = relations.into_iter().collect();
    }

    /// Whether this is a relationship this account has.
    #[must_use]
    pub fn knows(&self, relation: &str) -> bool {
        self.relations.contains(relation)
    }

    /// The devices this holds a mailbox for.
    #[must_use]
    pub fn devices_held(&self) -> Vec<Vec<u8>> {
        self.mailboxes.keys().cloned().collect()
    }

    /// Say which devices there are now, keeping what already waits for the ones that remain.
    ///
    /// **A device added must not cost somebody their post.** Adding a laptop is a routine act, and
    /// a mediator that started the account over on hearing about one would lose whatever was
    /// waiting for the phone. What goes is only the mailbox of a device that is no longer on the
    /// account — and it goes because there is nobody left who could collect it.
    pub fn devices(&mut self, devices: Vec<Vec<u8>>) {
        self.mailboxes.retain(|key, _| devices.contains(key));
        for key in devices {
            self.mailboxes.entry(key).or_default();
        }
    }

    /// Whether nobody has been for the post for long enough that this is not a mailbox any more.
    ///
    /// **Ninety days** (`SPECS.md §6.2`). The contents go; the warning is the app's to give when it
    /// reconnects, because a mediator has nobody to tell.
    #[must_use]
    pub fn inactive(&self, at: Epoch) -> bool {
        at.since(self.collected)
            .is_some_and(|gone| gone.0 >= quota::UNCOLLECTED_UNTIL_INACTIVE.0)
    }

    /// What has been turned away in this account's name, and since when.
    #[must_use]
    pub const fn turned_away(&self) -> Option<TurnedAway> {
        self.turned_away
    }

    /// What every mailbox is holding beyond the relationships' own reserves.
    ///
    /// **The reserves are not in it**, which is what makes them floors: what sits inside one was
    /// never in the shared total, so no other relationship can consume it.
    fn shared(&self) -> usize {
        let mut by_relation: BTreeMap<&str, usize> = BTreeMap::new();
        for message in self.mailboxes.values().flat_map(|one| one.held.iter()) {
            *by_relation.entry(message.relation.as_str()).or_default() += message.weighs();
        }
        by_relation
            .into_values()
            .map(quota::beyond_the_reserve)
            .sum()
    }

    /// What one relationship is holding across every mailbox.
    fn relation(&self, relation: &str) -> usize {
        self.mailboxes.values().map(|one| one.from(relation)).sum()
    }

    /// Take a message into one device's mailbox, or say why not.
    ///
    /// # Errors
    ///
    /// [`Refused`], which is what the sender is told and what the recipient is counted.
    pub fn deliver(&mut self, to: &[u8], message: Held, at: Epoch) -> Result<(), Refused> {
        self.room_for_one(to, &message, at)?;
        self.take(to, message);
        Ok(())
    }

    /// Whether there is room for that message in that mailbox, and nothing else.
    ///
    /// **Asked of every mailbox before anything is put in one**, because a delivery goes into all
    /// of them or into none. Counting a refusal here is what the recipient is owed: it happens once
    /// per delivery attempt, at the first mailbox with no room, and not once per device.
    ///
    /// # Errors
    ///
    /// [`Refused`], which is what the sender is told.
    pub fn room_for_one(&mut self, to: &[u8], message: &Held, at: Epoch) -> Result<(), Refused> {
        self.forget_the_expired(at);
        self.room_for(to, message, at).inspect_err(|why| {
            self.note(*why, at);
        })
    }

    /// Put it in, having established there is room.
    pub fn take(&mut self, to: &[u8], message: Held) {
        if let Some(mailbox) = self.mailboxes.get_mut(to) {
            mailbox.held.push(message);
        }
    }

    /// Whether there is room, without taking anything.
    fn room_for(&self, to: &[u8], message: &Held, at: Epoch) -> Result<(), Refused> {
        // **A mailbox nobody has come to for ninety days is not one** (`SPECS.md §6.2`). Taking
        // more for it would be holding post for somebody who has stopped existing as far as this
        // mediator can tell, and the person who finds out is the sender rather than the owner —
        // which is right, because the owner is not there to be told.
        if self.inactive(at) {
            return Err(Refused::Inactive);
        }
        if message.weighs() > quota::MESSAGE_MOST {
            return Err(Refused::TooLarge);
        }
        if !self.mailboxes.contains_key(to) {
            return Err(Refused::NoSuchMailbox);
        }
        // **A floor belongs to a relationship this account has**, and to nothing else. Somebody
        // writing under a name nobody has used is a stranger, and a stranger goes to the doorbell.
        if !self.knows(&message.relation) {
            return Err(Refused::NoSuchRelation);
        }

        let held = self.relation(&message.relation);
        if held + message.weighs() > quota::RELATION_MOST {
            return Err(Refused::RelationFull);
        }

        // **The reserve, applied.** What fits in this relationship's own floor is not counted
        // against the account at all — so a relationship that has used none of its floor takes a
        // small message however full every other relationship has made the total.
        let split = quota::splits(message.weighs(), held);
        if self.shared() + split.shared > quota::ACCOUNT_MOST {
            return Err(Refused::AccountFull);
        }
        Ok(())
    }

    /// Take a message addressed to the root identifier rather than to a relationship.
    ///
    /// **Cold contact and recovery, and nothing else** (`SPECS.md §6.5`). What that means is the
    /// client's to enforce — a mediator does not read its post — and what is enforced here is the
    /// separation itself and the small quota that goes with it.
    ///
    /// # Errors
    ///
    /// [`Refused::DoorbellFull`], [`Refused::TooLarge`], [`Refused::Inactive`].
    pub fn ring(&mut self, message: Held, at: Epoch) -> Result<(), Refused> {
        self.forget_the_expired(at);
        if self.inactive(at) {
            self.note(Refused::Inactive, at);
            return Err(Refused::Inactive);
        }
        if message.weighs() > quota::MESSAGE_MOST {
            self.note(Refused::TooLarge, at);
            return Err(Refused::TooLarge);
        }
        let held: usize = self.doorbell.iter().map(Held::weighs).sum();
        if held + message.weighs() > quota::DOORBELL_MOST {
            self.note(Refused::DoorbellFull, at);
            return Err(Refused::DoorbellFull);
        }
        self.doorbell.push(message);
        Ok(())
    }

    /// What is waiting in one device's mailbox, oldest first.
    #[must_use]
    pub fn waiting(&self, to: &[u8]) -> Option<&[Held]> {
        self.mailboxes.get(to).map(Mailbox::waiting)
    }

    /// What is waiting at the doorbell, oldest first.
    #[must_use]
    pub fn ringing(&self) -> &[Held] {
        &self.doorbell
    }

    /// Say that somebody came for the post, which is what keeps a mailbox a mailbox.
    pub fn collected(&mut self, at: Epoch) {
        self.collected = at;
    }

    /// Take those messages out, because they have been collected and confirmed.
    ///
    /// **Deletion after confirmed delivery** (`SPECS.md §6.2`), and confirmed is the word that
    /// matters: a mediator that deleted on handing over would lose a message to a connection that
    /// dropped between the two ends of one exchange.
    ///
    /// Names it does not hold are not an error. With several mediators a client confirms the same
    /// message to all of them, and only one of them had it first.
    pub fn confirm(&mut self, to: &[u8], names: &[Name], at: Epoch) {
        self.collected(at);
        if let Some(mailbox) = self.mailboxes.get_mut(to) {
            mailbox.held.retain(|one| !names.contains(&one.called));
        }
        self.doorbell.retain(|one| !names.contains(&one.called));
    }

    /// Drop what nobody may hold any longer.
    ///
    /// **Expiry is not the same as making room.** `SPECS.md §6.2` forbids dropping the old to fit
    /// the new, because that is how somebody evicts what matters by sending rubbish; this drops
    /// only what its own sender said had stopped being worth delivering.
    fn forget_the_expired(&mut self, at: Epoch) {
        for mailbox in self.mailboxes.values_mut() {
            mailbox.held.retain(|one| !one.expired(at));
        }
        self.doorbell.retain(|one| !one.expired(at));
    }

    /// Count one refusal, for the recipient to be told about.
    fn note(&mut self, _why: Refused, at: Epoch) {
        match &mut self.turned_away {
            Some(held) => held.count = held.count.saturating_add(1),
            None => {
                self.turned_away = Some(TurnedAway {
                    since: at,
                    count: 1,
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Account, Refused};
    use crate::held::Held;
    use crate::quota;
    use almena_format::identifier::Name;
    use almena_time::{Epoch, Epochs};

    /// One device's key, as an account carries it.
    fn device(mark: u8) -> Vec<u8> {
        let mut key = vec![0x02];
        key.extend_from_slice(&[mark; 32]);
        key
    }

    /// A message of that size, from that relationship.
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

    fn an_account() -> Account {
        let mut account = Account::of([device(1), device(2)], Epoch::GENESIS);
        account.relates(
            ["zRelation", "zFlooder", "zQuiet", "zFirst"]
                .into_iter()
                .map(str::to_owned),
        );
        account
    }

    #[test]
    fn a_message_waits_in_the_mailbox_it_was_delivered_to_and_in_no_other() {
        // One mailbox per device, because deletion after collection has to work per mailbox: with
        // one shared between a phone and a laptop, two devices collecting at once would race.
        let mut account = an_account();
        account
            .deliver(&device(1), message(1, "zRelation", 100), Epoch::GENESIS)
            .expect("there is room");

        assert_eq!(account.waiting(&device(1)).expect("a mailbox").len(), 1);
        assert!(account.waiting(&device(2)).expect("a mailbox").is_empty());
    }

    #[test]
    fn flooding_one_relationship_does_not_take_another_relationship_s_reserve() {
        // **The exit criterion's second clause**, and the reason the reserve exists. Without it the
        // way to silence somebody is to write to them: one counterparty fills the account's total
        // and every other relationship goes mute with its own channel empty.
        let mut account = an_account();

        // One relationship writes as much as the account will hold in the shared part.
        let mut sent = 0usize;
        let each = quota::RESERVED_MESSAGE_MOST * 4;
        while sent < quota::ACCOUNT_MOST + quota::RELATION_MOST {
            let outcome = account.deliver(&device(1), message(2, "zFlooder", each), Epoch::GENESIS);
            if outcome.is_err() {
                break;
            }
            sent += each;
        }
        assert!(sent > 0, "it got some of it in");
        assert!(
            account
                .deliver(&device(1), message(3, "zFlooder", each), Epoch::GENESIS)
                .is_err(),
            "and then it is full, on its own channel or on the account's"
        );

        // And the relationship that has written nothing still gets a small message through.
        account
            .deliver(&device(1), message(4, "zQuiet", 1_000), Epoch::GENESIS)
            .expect("its own floor is untouched");
    }

    #[test]
    fn flooding_a_relationship_does_not_block_the_doorbell() {
        // **The exit criterion's second clause, other half.** The doorbell is a channel of its own
        // with a quota of its own, so what fills the mailboxes never reaches it — which is what
        // keeps recovery arriving when everything else is blocked.
        let mut account = an_account();
        let each = quota::RESERVED_MESSAGE_MOST * 4;
        while account
            .deliver(&device(1), message(2, "zFlooder", each), Epoch::GENESIS)
            .is_ok()
        {}

        account
            .ring(message(5, "zStranger", 500), Epoch::GENESIS)
            .expect("the doorbell is its own channel");
    }

    #[test]
    fn a_full_doorbell_costs_introductions_and_touches_no_relationship() {
        // The other direction, which is the point of separating them: whoever fills the doorbell
        // takes away meeting somebody new, and nothing that is already established.
        let mut account = an_account();
        while account
            .ring(message(6, "zStranger", 8_000), Epoch::GENESIS)
            .is_ok()
        {}
        assert_eq!(
            account.ring(message(7, "zStranger", 8_000), Epoch::GENESIS),
            Err(Refused::DoorbellFull)
        );

        account
            .deliver(&device(1), message(8, "zRelation", 1_000), Epoch::GENESIS)
            .expect("a relationship is untouched by a full doorbell");
    }

    #[test]
    fn what_was_turned_away_is_counted_for_the_recipient() {
        // **Not a courtesy** (`SPECS.md §6.5`): a refusal that goes only to the sender leaves the
        // person being attacked looking at silence, unable to tell it from nobody writing. That is
        // what makes the attack invisible, and §1.2 says denying service yes, hiding it no.
        let mut account = an_account();
        assert!(account.turned_away().is_none(), "nothing yet");

        let too_large = message(9, "zRelation", quota::MESSAGE_MOST + 1);
        assert_eq!(
            account.deliver(&device(1), too_large, Epoch::new(5)),
            Err(Refused::TooLarge)
        );

        let told = account.turned_away().expect("and now there is");
        assert_eq!(told.since, Epoch::new(5), "from when");
        assert_eq!(told.count, 1, "and how many");
    }

    #[test]
    fn nothing_old_is_dropped_to_make_room_for_something_new() {
        // `SPECS.md §6.2`: dropping the old to fit the new is how somebody evicts what matters by
        // sending rubbish. A full mailbox refuses; it never tidies.
        let mut account = an_account();
        account
            .deliver(&device(1), message(1, "zFirst", 1_000), Epoch::GENESIS)
            .expect("room");
        let first = account.waiting(&device(1)).expect("a mailbox")[0]
            .called
            .clone();

        let each = quota::RESERVED_MESSAGE_MOST * 4;
        while account
            .deliver(&device(1), message(2, "zFlooder", each), Epoch::GENESIS)
            .is_ok()
        {}

        assert!(
            account
                .waiting(&device(1))
                .expect("a mailbox")
                .iter()
                .any(|one| one.called == first),
            "the first message is still there"
        );
    }

    #[test]
    fn what_is_collected_and_confirmed_is_what_goes() {
        // Deletion after *confirmed* delivery, so a connection that dropped mid-exchange does not
        // cost a message. And a name this mediator never held is not an error: with several
        // mediators the client confirms the same message to all of them.
        let mut account = an_account();
        account
            .deliver(&device(1), message(1, "zRelation", 100), Epoch::GENESIS)
            .expect("room");
        let held = account.waiting(&device(1)).expect("a mailbox")[0]
            .called
            .clone();

        account.confirm(&device(1), &[Name::of(b"never held")], Epoch::new(3));
        assert_eq!(account.waiting(&device(1)).expect("a mailbox").len(), 1);

        account.confirm(&device(1), &[held], Epoch::new(4));
        assert!(account.waiting(&device(1)).expect("a mailbox").is_empty());
    }

    #[test]
    fn a_message_is_held_no_longer_than_its_sender_asked_and_no_longer_than_the_ceiling() {
        let mut account = an_account();
        let long = Held::new(
            "zRelation".to_owned(),
            vec![7u8; 10],
            Epochs(100_000),
            Epoch::GENESIS,
        );
        assert_eq!(
            long.until.number(),
            quota::HELD_AT_MOST.0,
            "asked for longer, held to the ceiling"
        );

        account
            .deliver(&device(1), long, Epoch::GENESIS)
            .expect("room");
        account
            .deliver(&device(1), message(2, "zRelation", 10), Epoch::GENESIS)
            .expect("room");

        // A moment past the shorter one's deadline, and it is the only one gone.
        account
            .deliver(&device(1), message(3, "zRelation", 10), Epoch::new(25))
            .expect("room");
        assert_eq!(account.waiting(&device(1)).expect("a mailbox").len(), 2);
    }

    #[test]
    fn a_mailbox_nobody_comes_to_stops_being_one() {
        // Ninety days (`SPECS.md §6.2`). The warning is the app's to give when it reconnects,
        // because a mediator has nobody to tell.
        let mut account = an_account();
        let long_after = Epoch::new(quota::UNCOLLECTED_UNTIL_INACTIVE.0);
        assert!(account.inactive(long_after));
        assert_eq!(
            account.deliver(&device(1), message(1, "zRelation", 10), long_after),
            Err(Refused::Inactive)
        );

        // And coming for the post is what keeps it one.
        account.collected(long_after);
        account
            .deliver(&device(1), message(2, "zRelation", 10), long_after)
            .expect("collected just now");
    }

    #[test]
    fn a_relationship_this_account_does_not_have_gets_no_floor_of_its_own() {
        // **A floor anybody could claim is not a floor.** If writing under a name nobody has used
        // bought a reserve, inventing names would spend the account's whole ceiling from outside.
        // Somebody with no relationship has the doorbell, and that is what it is for.
        let mut account = an_account();
        assert_eq!(
            account.deliver(&device(1), message(1, "zStranger", 100), Epoch::GENESIS),
            Err(Refused::NoSuchRelation)
        );
        account
            .ring(message(2, "zStranger", 100), Epoch::GENESIS)
            .expect("the doorbell is where a stranger goes");
    }

    #[test]
    fn a_mailbox_this_mediator_does_not_have_is_said_and_not_invented() {
        let mut account = an_account();
        assert_eq!(
            account.deliver(&device(9), message(1, "zRelation", 10), Epoch::GENESIS),
            Err(Refused::NoSuchMailbox)
        );
    }
}
