//! `revoke`: flip the bit, publish the version, and tell the holder.
//!
//! Three things, and the third is a duty: an issuer that turns a bit off tells the holder over the
//! relationship they already have, or the holder finds out at a counter. The list is republished
//! under the element's own key, because revoking has to cost what issuing costs.

use almena_credential::Status;
use almena_format::identifier::{Did, Name};
use almena_sdk::{errand, issuer};
use almena_status::list::List;
use almena_suite::ed25519;
use almena_time::Epoch;

use crate::chain;
use crate::commands::{Partner, signed_by};
use crate::failed::Failed;
use crate::issued::Record;

/// What `revoke` leaves behind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Revoked {
    /// The list, by identifier.
    pub list: String,
    /// Which bit was set.
    pub index: u64,
    /// The epoch the version was published in.
    pub at: u64,
    /// Whether the holder's mediators took the notice.
    pub told: bool,
}

/// Revoke a credential this partner issued, by its identifier.
///
/// # Errors
///
/// `revoke_not_issued_here`, `revoke_not_revocable`, `revoke_list_unknown`, and what the record or
/// the node fails with. The notice not landing is not one of them: the version is published
/// whatever the holder's mediators say, and `told` says whether they took it.
pub async fn run(
    partner: &Partner,
    identifier: &str,
    issuer_key: [u8; 32],
) -> Result<Revoked, Failed> {
    let mut issued = partner.directory.issued()?;
    let record = issued
        .get(identifier)
        .cloned()
        .ok_or_else(|| Failed::new("revoke_not_issued_here"))?;
    let (Some(list), Some(index)) = (record.list.clone(), record.index) else {
        return Err(Failed::new("revoke_not_revocable"));
    };
    let network = chain::network(&partner.node).await?;
    let republishing = Republishing {
        list: Did::parse(&list).map_err(|_| Failed::new("revoke_list_unknown"))?,
        index,
        issuer_key,
        at: network.epoch,
    };
    bit_set(partner, &republishing).await?;

    if let Some(record) = issued.get_mut(identifier) {
        record.revoked_at = Some(network.epoch);
    }
    partner.directory.keep_issued(&issued)?;
    log::info!("credential_revoked credential={identifier} list={list} index={index}");

    let status = Status::Revocable {
        list: list.clone(),
        index,
    };
    let told = told(partner, identifier, &record, status, network.epoch).await;
    Ok(Revoked {
        list,
        index,
        at: network.epoch,
        told,
    })
}

/// One bit to set, and what publishing the version that carries it needs.
struct Republishing {
    list: Did,
    index: u64,
    issuer_key: [u8; 32],
    at: u64,
}

/// Set the bit in the list this partner keeps for that cohort, and publish the version.
async fn bit_set(partner: &Partner, republishing: &Republishing) -> Result<(), Failed> {
    let mut lists = partner.directory.lists()?;
    let list = republishing.list.to_string();
    let cohort = lists
        .cohort_of(&list)
        .ok_or_else(|| Failed::new("revoke_list_unknown"))?
        .to_owned();
    let mut held = lists
        .get(&cohort)
        .cloned()
        .ok_or_else(|| Failed::new("revoke_list_unknown"))?;
    let mut bits = held.bits()?;
    bits.revoke(republishing.index);
    let previous = Name::parse(&held.previous).map_err(|_| Failed::new("revoke_list_unknown"))?;
    held.previous = republished(partner, republishing, previous, &bits).await?;
    held.written = bits.written();
    lists.keep(&cohort, held);
    partner.directory.keep_lists(&lists)
}

/// The next version of the list, in the record and on the node; the name of the act that put it
/// there, which the version after this one follows.
async fn republished(
    partner: &Partner,
    republishing: &Republishing,
    previous: Name,
    bits: &List,
) -> Result<String, Failed> {
    let on = &republishing.list;
    let by = chain::status_list(&partner.node, on).await?.by;
    let mut act = issuer::republishing(bits, on, previous, Epoch::new(republishing.at));
    signed_by(
        &mut act,
        &ed25519::SigningKey::from_secret(republishing.issuer_key),
        &by,
    );
    chain::deliver(&partner.node, &act).await?;
    chain::keep_list(&partner.node, bits).await?;
    log::info!(
        "list_republished list={on} version={} revoked={}",
        crate::directory::hex(bits.version().bytes()),
        bits.how_many()
    );
    Ok(act.called().as_str().to_owned())
}

/// Tell the holder, over the relationship the credential was offered on.
async fn told(
    partner: &Partner,
    identifier: &str,
    record: &Record,
    status: Status,
    at: u64,
) -> bool {
    let Ok(relations) = partner.directory.relations() else {
        return false;
    };
    let Some(relation) = relations.whose_far_end_is(&record.relation) else {
        log::info!("revoked_notice_no_relationship credential={identifier}");
        return false;
    };
    let Ok(body) = errand::revoked(identifier, &status, Epoch::new(at)) else {
        return false;
    };
    match partner
        .send(relation, errand::kind::REVOKED, identifier, body)
        .await
    {
        Ok(delivered) => {
            log::info!(
                "revoked_notice_delivered credential={identifier} reached={}",
                delivered.reached
            );
            true
        }
        Err(why) => {
            log::info!("revoked_notice_not_delivered credential={identifier} reason={why}");
            false
        }
    }
}
