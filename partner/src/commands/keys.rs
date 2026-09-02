//! `keys`: make or load the partner's keys, and put its account on the record.
//!
//! **A partner is a holder account too.** To own or manage an organisation it needs a root
//! identifier, and to sign as an owner it needs a P-256 device key — exactly what the holder's
//! app makes for a person. So this composes the same two acts a wallet composes: the creation,
//! signed by the control key it establishes, and one device added, signed by the same key.
//!
//! Run twice, it makes nothing twice: keys already there are read back, and an account already
//! written down is printed again.

use std::collections::BTreeMap;

use almena_format::cbor::Value;
use almena_format::identifier::{Did, Network};
use almena_format::operation::{Operation, create};
use almena_store::kind::Kind;
use almena_time::Epoch;

use crate::chain;
use crate::commands::{Partner, signed_by};
use crate::directory::{Directory, Keys, hex};
use crate::failed::Failed;

/// What `keys` leaves behind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Made {
    /// The account's identifier.
    pub account: Did,
    /// The device's public key, compressed, as hexadecimal.
    pub device: String,
    /// The issuer element's public key, the 32 Ed25519 bytes, as hexadecimal: what the element
    /// form takes.
    pub element: String,
    /// The issuance public key, the 33 compressed P-256 bytes, as hexadecimal: what
    /// `ISSUER_SET_ISSUANCE_KEY` takes.
    pub issuance: String,
    /// Whether the account was put on the record by this run, or was there already.
    pub submitted: bool,
}

/// Make or load the keys, and put the account on the record if it is not there yet.
///
/// The element's keys are made in the same run and never submitted: the element is created by
/// its owners from the public key printed here, on a screen this program has no part in.
///
/// # Errors
///
/// What the directory or the node fails with; `act_not_taken` with the node's rule where the
/// account or the device was refused.
pub async fn run(partner: &Partner) -> Result<Made, Failed> {
    let (keys, made) = partner.directory.keys()?;
    let control = keys.control_key();
    let device = keys.device_key()?;
    let device_hex = hex(&device.verifying_key().bytes());
    log::info!(
        "keys_{} control={} device={device_hex}",
        if made { "made" } else { "read" },
        hex(&control.verifying_key().bytes())
    );
    let (element, issuance) = element_made(&partner.directory)?;

    if let Some(account) = partner.directory.account()? {
        log::info!("account_held account={account}");
        return Ok(Made {
            account,
            device: device_hex,
            element,
            issuance,
            submitted: false,
        });
    }

    let network = chain::network(&partner.node).await?;
    let account = submitted(partner, &keys, network.which, Epoch::new(network.epoch)).await?;
    log::info!("device_submitted account={account} device={device_hex}");

    partner.directory.keep_account(&account)?;
    Ok(Made {
        account,
        device: device_hex,
        element,
        issuance,
        submitted: true,
    })
}

/// The element's keys, made or read back, as the two public halves in hexadecimal.
fn element_made(directory: &Directory) -> Result<(String, String), Failed> {
    let (keys, made) = directory.element_keys()?;
    let element = hex(&keys.element_key().verifying_key().bytes());
    let issuance = hex(&keys.issuance_key()?.verifying_key().bytes());
    log::info!(
        "element_keys_{} element={element} issuance={issuance}",
        if made { "made" } else { "read" }
    );
    Ok((element, issuance))
}

/// The two acts, composed, signed by the control key and handed over: the creation, then the
/// device.
async fn submitted(
    partner: &Partner,
    keys: &Keys,
    network: Network,
    at: Epoch,
) -> Result<Did, Failed> {
    let control = keys.control_key();
    let device = keys.device_key()?;
    let mut created = create(
        network,
        Kind::HOLDER_CREATE.number(),
        1,
        at,
        BTreeMap::from([(1, Value::Bytes(control.verifying_key().bytes().to_vec()))]),
    );
    let account = created.object.clone();
    signed_by(&mut created, &control, &account);
    chain::deliver(&partner.node, &created).await?;
    log::info!("account_submitted account={account} epoch={}", at.number());

    let mut adding = Operation {
        object: account.clone(),
        previous: Some(created.called()),
        kind: Kind::HOLDER_ADD_DEVICE.number(),
        version: 1,
        issued: at,
        payload: BTreeMap::from([(1, Value::Bytes(device.verifying_key().bytes().to_vec()))]),
        signatures: Vec::new(),
    };
    signed_by(&mut adding, &control, &account);
    chain::deliver(&partner.node, &adding).await?;
    Ok(account)
}
