//! What an issuer and a holder say to each other, and the one message that has to be sent.
//!
//! # Three messages, and only one of them is a duty
//!
//! `SPECS.md §6.4` names the cases. Offering a credential and acknowledging what was decided about
//! it are the ordinary two. The third is different: **when an issuer publishes a version of a status
//! list that turns a bit off, it tells the holder** — over the relationship they already have — and
//! `SPECS.md §10.2` makes that a conformance requirement, like processing a rotation.
//!
//! Without it there is no way back: the issuer turns a bit off and the holder finds out when a
//! verifier refuses them, at the counter.
//!
//! # And it comes from the issuer's own records, not from the list
//!
//! The list does not say whose each index is. What the message is derived from is the
//! index↔holder correspondence the issuer keeps anyway — **which this document requires a third
//! party to keep for the first time**, and that is worth saying rather than leaving implicit.
//!
//! > It does not reach somebody with no live relationship. A credential issued to a person who has
//! > since rotated without confirming, or who never opened a relationship with that issuer, is
//! > revoked with nobody able to tell them. That is `SPECS.md §17.16`, and there is no fix inside
//! > the design: telling them through the doorbell would need the issuer to keep every holder's
//! > root identifier, which is exactly the link `SPECS.md §3` exists not to create.

use almena_credential::Status;
use almena_time::Epoch;

/// What each message is called on the wire.
pub mod kind {
    /// An issuer offering a credential.
    pub const OFFER: &str = "https://almena.network/credential/offer/1.0";
    /// The holder saying what was decided about one.
    pub const DECIDED: &str = "https://almena.network/credential/decided/1.0";
    /// An issuer telling a holder it has revoked one.
    pub const REVOKED: &str = "https://almena.network/credential/revoked/1.0";
}

/// How a credential came to be offered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Came {
    /// The holder asked for it.
    Asked,
    /// It arrived on its own, after something done in person or on the issuer's own initiative.
    Unasked,
    /// It replaces one that is running out.
    Renewal,
}

impl Came {
    /// The name it travels as.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Asked => "asked",
            Self::Unasked => "unasked",
            Self::Renewal => "renewal",
        }
    }
}

/// The body of an offer.
///
/// **The deadline is the issuer's and is bounded by the mailbox's** (`SPECS.md §6.2`, thirty days).
/// What it decides is how long a holder who refused by mistake can take it back before the only
/// remedy is a reissue.
#[must_use]
pub fn offering(
    credential: &str,
    came: Came,
    until: Epoch,
    renews: Option<&str>,
) -> serde_json::Value {
    serde_json::json!({
        "credential": credential,
        "came": came.name(),
        "until": until.number(),
        // **Named where there is one** (`SPECS.md §9.5`): renewing is issuing, and the one
        // difference is that the offer says which credential it takes the place of.
        "renews": renews,
    })
}

/// The body of an acknowledgement.
///
/// **Sent either way** (`SPECS.md §9.5`). Refusing in silence leaves the issuer believing delivery
/// is still pending, and accepting in silence leaves it the same.
#[must_use]
pub fn decided(credential: &str, taken: bool) -> serde_json::Value {
    serde_json::json!({ "credential": credential, "taken": taken })
}

/// There is no notice to send about a credential that says it cannot be revoked.
///
/// **Its own type rather than nothing**, because *nothing to send* and *something went wrong* are
/// different answers and a caller that could not tell them apart would treat one as the other.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotRevocable;

/// The body of a revocation notice.
///
/// # Errors
///
/// [`NotRevocable`] for a credential that says it cannot be revoked — composing a notice about one
/// would be telling a holder something the credential itself contradicts.
pub fn revoked(
    credential: &str,
    status: &Status,
    at: Epoch,
) -> Result<serde_json::Value, NotRevocable> {
    let Status::Revocable { list, index } = status else {
        return Err(NotRevocable);
    };
    Ok(serde_json::json!({
        "credential": credential,
        "list": list,
        "index": index,
        "at": at.number(),
    }))
}

#[cfg(test)]
mod tests {
    use super::{Came, NotRevocable, decided, offering, revoked};
    use almena_credential::Status;
    use almena_time::Epoch;

    #[test]
    fn an_offer_says_how_it_came_and_what_it_replaces() {
        // An unsolicited credential from an unknown issuer is a different situation from the one
        // asked for a minute ago, and the message is where that difference starts.
        let plain = offering("a~b~", Came::Unasked, Epoch::new(1_000), None);
        assert_eq!(plain["came"], "unasked");
        assert_eq!(plain["renews"], serde_json::Value::Null);

        let again = offering(
            "a~b~",
            Came::Renewal,
            Epoch::new(1_000),
            Some("the-old-one"),
        );
        assert_eq!(again["came"], "renewal");
        assert_eq!(again["renews"], "the-old-one");
    }

    #[test]
    fn an_acknowledgement_is_sent_either_way() {
        // Refusing in silence leaves the issuer believing delivery is still pending.
        assert_eq!(decided("one", true)["taken"], true);
        assert_eq!(decided("one", false)["taken"], false);
    }

    #[test]
    fn a_notice_names_the_list_and_the_place_in_it() {
        // So that the holder can go and check rather than take the issuer's word: the list is
        // public and addressed by hash, and the record says which version is current.
        let notice = revoked(
            "one",
            &Status::Revocable {
                list: "did:almena:dev:zAList".to_owned(),
                index: 4242,
            },
            Epoch::new(1_100),
        )
        .expect("a notice");
        assert_eq!(notice["list"], "did:almena:dev:zAList");
        assert_eq!(notice["index"], 4242);
        assert_eq!(notice["at"], 1_100);
    }

    #[test]
    fn there_is_no_notice_to_send_about_something_that_cannot_be_revoked() {
        // Composing one would be telling a holder something the credential itself contradicts.
        assert_eq!(
            revoked("one", &Status::NotRevocable, Epoch::new(1_100)),
            Err(NotRevocable)
        );
    }
}
