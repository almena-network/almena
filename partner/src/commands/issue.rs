//! `issue`: sign a credential against a template and offer it to a holder.
//!
//! **The issuer decides what goes in; the template decides what may.** The attributes come from
//! the operator — the command line or a file — and `almena_sdk::issuer::issue` holds them to the
//! template version the credential names. Nothing here fills in a value.
//!
//! # Two keys, and which signs what
//!
//! The **issuance key** is the P-256 key the record says the issuer element emits with, set by the
//! owners in `ISSUER_SET_ISSUANCE_KEY`; it signs the credential. The **element's own key** is the
//! Ed25519 key the element was created with; it signs the acts that publish status list versions,
//! because revoking has to cost what issuing costs and not a meeting of the owners.
//!
//! # What the credential is bound to
//!
//! The key the holder signs with in this relationship, which their peer identifier carries. A
//! wallet makes one key per credential when it accepts, and no message in this version carries
//! that key back to the issuer before signing — so the one key of the holder's an issuer holds is
//! the relationship's, and that is what the confirmation names. Said here rather than left for the
//! wallet to discover at presentation.

use std::collections::BTreeMap;

use almena_credential::Status;
use almena_format::identifier::{Did, Name};
use almena_sdk::errand::{self, Came};
use almena_sdk::issuer::{self, Issuing};
use almena_status::list::{AT_LEAST, List};
use almena_suite::{ed25519, p256};
use almena_time::Epoch;
use almena_time::cohort::Cohort;

use crate::chain::{self, Opened};
use crate::commands::{Partner, drawn_name, signed_by};
use crate::directory::Directory;
use crate::failed::Failed;
use crate::issued::Record;
use crate::lists::Held;
use crate::relations::Relation;

/// How long an offer stands before the holder's refusal is final, in epochs: thirty days, which
/// is the longest a mediator holds anything.
pub const OFFER_STANDS: u64 = 720;

/// What an operator asks to be issued.
#[derive(Debug, Clone)]
pub struct Asked {
    /// The far end of the relationship the offer goes on.
    pub to: String,
    /// The issuer element the credential is issued by.
    pub issuer: Did,
    /// The P-256 secret the element emits with.
    pub issuance_key: [u8; 32],
    /// The Ed25519 secret of the element itself, which signs status list acts.
    pub issuer_key: [u8; 32],
    /// The template version, by the hash of the act that published it.
    pub template: Name,
    /// What it says, by attribute.
    pub attributes: BTreeMap<Name, serde_json::Value>,
    /// The epoch it stops being valid in.
    pub expires: u64,
    /// Whether it can be revoked, which means it carries an index in a list.
    pub revocable: bool,
    /// The credential's own identifier, or nothing to have one drawn.
    pub identifier: Option<String>,
    /// How it came to be offered.
    pub came: Came,
    /// Which credential it renews, where it renews one.
    pub renews: Option<String>,
}

/// What `issue` leaves behind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offered {
    /// The credential's identifier.
    pub identifier: String,
    /// Where its bit is, where it has one.
    pub status: Status,
    /// How many of the holder's mediators took the offer.
    pub reached: usize,
}

/// Issue and offer.
///
/// # Errors
///
/// `issue_not_a_relationship`, `issue_not_against_template` naming the attribute, `issue_not_signed`,
/// and what the record or the node fails with.
pub async fn run(partner: &Partner, asked: &Asked) -> Result<Offered, Failed> {
    let relations = partner.directory.relations()?;
    let relation = relations
        .whose_far_end_is(&asked.to)
        .ok_or_else(|| Failed::new("issue_not_a_relationship"))?;
    let network = chain::network(&partner.node).await?;
    let (_, version) = chain::template_version(&partner.node, &asked.template).await?;

    let status = if asked.revocable {
        placed(partner, asked, &network).await?
    } else {
        Status::NotRevocable
    };
    let identifier = match &asked.identifier {
        Some(identifier) => identifier.clone(),
        None => drawn_name()?,
    };
    let written = signed(
        asked,
        &version,
        &Signing {
            identifier: &identifier,
            holder: holder_key(relation)?,
            status: &status,
            epoch: network.epoch,
        },
    )?;
    log::info!(
        "credential_issued credential={identifier} template={} attributes={} revocable={}",
        asked.template.as_str(),
        asked.attributes.len(),
        asked.revocable
    );

    let until = Epoch::new(network.epoch.saturating_add(OFFER_STANDS));
    let body = errand::offering(&written, asked.came, until, asked.renews.as_deref());
    let delivered = partner
        .send(relation, errand::kind::OFFER, &identifier, body)
        .await?;
    kept(partner, asked, &identifier, written, &status)?;
    log::info!(
        "offer_delivered credential={identifier} reached={}",
        delivered.reached
    );
    Ok(Offered {
        identifier,
        status,
        reached: delivered.reached,
    })
}

/// What one signing needs beyond what was asked: the name, the key, the place and the moment.
struct Signing<'a> {
    identifier: &'a str,
    holder: p256::VerifyingKey,
    status: &'a Status,
    epoch: u64,
}

/// The credential, signed against the template version under the issuance key.
fn signed(
    asked: &Asked,
    version: &almena_store::template::Version,
    signing: &Signing<'_>,
) -> Result<String, Failed> {
    let key = p256::SigningKey::from_secret(asked.issuance_key)
        .map_err(|_| Failed::new("issue_issuance_key_invalid"))?;
    let issued = issuer::issue(
        &Issuing {
            issuer: &asked.issuer,
            template: version,
            identifier: signing.identifier,
            attributes: &asked.attributes,
            holder: &signing.holder,
            between: (Epoch::new(signing.epoch), Epoch::new(asked.expires)),
            status: signing.status.clone(),
        },
        &key,
    )
    .map_err(|why| Failed::with("issue_not_against_template", "why", &format!("{why:?}")))?;
    Ok(issued.written())
}

/// The issuer element to issue as: the one named now, or the one named last time.
///
/// **Typed once.** An organisation's program issues as one element for years; asking for its
/// identifier on every run would have operators keep it in a shell history instead of here.
/// Naming a different one replaces what is remembered, once that issue has gone through.
///
/// # Errors
///
/// `issue_not_a_did` for text that is not an identifier; `issue_no_issuer` when none was given
/// and none is remembered.
pub fn issuer_of(directory: &Directory, given: Option<&str>) -> Result<Did, Failed> {
    match given {
        Some(text) => Did::parse(text).map_err(|_| Failed::with("issue_not_a_did", "issuer", text)),
        None => directory
            .issuer()?
            .ok_or_else(|| Failed::new("issue_no_issuer")),
    }
}

/// Write the credential down beside the relationship and the place its bit has, and the issuer
/// beside the account so the next run need not name it.
fn kept(
    partner: &Partner,
    asked: &Asked,
    identifier: &str,
    written: String,
    status: &Status,
) -> Result<(), Failed> {
    if partner.directory.issuer()?.as_ref() != Some(&asked.issuer) {
        partner.directory.keep_issuer(&asked.issuer)?;
        log::info!("issuer_remembered issuer={}", asked.issuer);
    }
    let mut all = partner.directory.issued()?;
    let (list, index) = match status {
        Status::Revocable { list, index } => (Some(list.clone()), Some(*index)),
        Status::NotRevocable => (None, None),
    };
    all.keep(
        identifier,
        Record {
            written,
            relation: asked.to.clone(),
            list,
            index,
            decided: None,
            revoked_at: None,
        },
    );
    partner.directory.keep_issued(&all)
}

/// The key the credential is bound to: the holder's signing key in this relationship.
fn holder_key(relation: &Relation) -> Result<p256::VerifyingKey, Failed> {
    let far = relation.far_end()?;
    let key: [u8; p256::PUBLIC_KEY_WIDTH] = far
        .signs
        .first()
        .and_then(|key| key.as_slice().try_into().ok())
        .ok_or_else(|| Failed::new("issue_holder_has_no_key"))?;
    p256::VerifyingKey::from_bytes(key).map_err(|_| Failed::new("issue_holder_has_no_key"))
}

/// A place in the list for the cohort the credential expires in, opening the list if it must.
async fn placed(partner: &Partner, asked: &Asked, network: &Opened) -> Result<Status, Failed> {
    let clock = network.clock()?;
    let cohort = Cohort::of(&clock, Epoch::new(asked.expires))
        .ok_or_else(|| Failed::new("issue_expiry_out_of_reach"))?;
    let mut lists = partner.directory.lists()?;
    let held = match lists.get(&cohort.written()) {
        Some(held) => held.clone(),
        None => {
            let held = opened(partner, asked, network, cohort).await?;
            lists.keep(&cohort.written(), held.clone());
            partner.directory.keep_lists(&lists)?;
            held
        }
    };
    let issued = partner.directory.issued()?;
    let entries = held.bits()?.entries().max(AT_LEAST);
    for _ in 0..64 {
        let index = almena_status::list::somewhere(entries)
            .map_err(|_| Failed::new("partner_no_entropy"))?;
        if !issued.taken(&held.list, index) {
            return Ok(Status::Revocable {
                list: held.list.clone(),
                index,
            });
        }
    }
    Err(Failed::new("issue_list_full"))
}

/// Open a status list for a cohort: an empty list, its first version in the record, its bytes
/// on the node.
async fn opened(
    partner: &Partner,
    asked: &Asked,
    network: &Opened,
    cohort: Cohort,
) -> Result<Held, Failed> {
    let list = List::empty();
    let mut act = issuer::publishing(
        network.which,
        &list,
        &asked.issuer,
        cohort,
        Epoch::new(network.epoch),
    );
    signed_by(
        &mut act,
        &ed25519::SigningKey::from_secret(asked.issuer_key),
        &asked.issuer,
    );
    chain::deliver(&partner.node, &act).await?;
    chain::keep_list(&partner.node, &list).await?;
    log::info!(
        "list_published list={} cohort={} version={}",
        act.object,
        cohort.written(),
        crate::directory::hex(list.version().bytes())
    );
    Ok(Held {
        list: act.object.to_string(),
        previous: act.called().as_str().to_owned(),
        written: list.written(),
    })
}

/// One attribute as the command line writes it: `name=value`, the value JSON where it is JSON
/// and text where it is not.
///
/// **Nothing is guessed about a value's type beyond that.** `true` is a yes, `42` is a number,
/// `"42"` is the text; whether the template wanted a yes where text was given is the library's
/// refusal to make.
///
/// # Errors
///
/// `issue_attribute_unreadable` for text that is not `name=value` or a name that is not one.
pub fn attribute(written: &str) -> Result<(Name, serde_json::Value), Failed> {
    let (name, value) = written
        .split_once('=')
        .ok_or_else(|| Failed::with("issue_attribute_unreadable", "given", written))?;
    let name = Name::parse(name.trim())
        .map_err(|_| Failed::with("issue_attribute_unreadable", "name", name.trim()))?;
    let value = serde_json::from_str(value.trim())
        .unwrap_or_else(|_| serde_json::Value::String(value.trim().to_owned()));
    Ok((name, value))
}

/// Attributes out of a JSON file an operator wrote: an object of name to value.
///
/// # Errors
///
/// `issue_attributes_file_unreadable`.
pub fn attributes_in(text: &str) -> Result<BTreeMap<Name, serde_json::Value>, Failed> {
    let held: serde_json::Map<String, serde_json::Value> =
        serde_json::from_str(text).map_err(|_| Failed::new("issue_attributes_file_unreadable"))?;
    held.into_iter()
        .map(|(name, value)| {
            Name::parse(&name)
                .map(|name| (name, value))
                .map_err(|_| Failed::with("issue_attributes_file_unreadable", "name", &name))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{attribute, attributes_in, issuer_of};
    use crate::directory::Directory;
    use almena_format::identifier::{Did, Name};

    #[test]
    fn the_issuer_named_now_wins_and_the_one_named_last_time_is_the_default() {
        let path =
            std::env::temp_dir().join(format!("almena-partner-issuer-of-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        let directory = Directory::at(&path).expect("a directory");
        let account = Did::parse("did:almena:dev:zQmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG")
            .expect("a did");
        let issuer = Did::parse("did:almena:dev:zQmZ56DfvnAoStjoSnF4jUK5LoZNE9T9k7z5nQGWvao1CRT")
            .expect("a did");
        assert_eq!(
            issuer_of(&directory, None).unwrap_err().to_string(),
            "issue_no_issuer"
        );
        assert!(
            issuer_of(&directory, Some("not a did"))
                .unwrap_err()
                .to_string()
                .starts_with("issue_not_a_did")
        );
        assert_eq!(
            issuer_of(&directory, Some(&issuer.to_string())).expect("named"),
            issuer
        );
        directory.keep_account(&account).expect("kept");
        directory.keep_issuer(&issuer).expect("kept");
        assert_eq!(issuer_of(&directory, None).expect("remembered"), issuer);
        let _ = std::fs::remove_dir_all(&path);
    }

    #[test]
    fn a_value_is_json_where_it_is_json_and_text_where_it_is_not() {
        let name = Name::of(b"an attribute");
        let (_, yes) = attribute(&format!("{}=true", name.as_str())).expect("read");
        assert_eq!(yes, serde_json::json!(true));
        let (_, date) = attribute(&format!("{}=1815-12-10", name.as_str())).expect("read");
        assert_eq!(date, serde_json::json!("1815-12-10"));
        let (_, quoted) = attribute(&format!("{}=\"42\"", name.as_str())).expect("read");
        assert_eq!(quoted, serde_json::json!("42"));
        assert!(attribute("no-equals").is_err());
        assert!(attribute("notaname=1").is_err());
    }

    #[test]
    fn a_file_is_an_object_of_names_and_nothing_else() {
        let name = Name::of(b"an attribute");
        let read = attributes_in(&format!("{{\"{}\": \"Ada\"}}", name.as_str())).expect("read");
        assert_eq!(read.get(&name), Some(&serde_json::json!("Ada")));
        assert!(attributes_in("[1,2]").is_err());
        assert!(attributes_in("{\"notaname\": 1}").is_err());
    }
}
