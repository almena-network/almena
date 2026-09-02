//! What the program is asked to do, one errand per subcommand.
//!
//! Each errand is a function over a [`Partner`] — the directory and the node — so that a test
//! walks the whole cycle by calling them against a node it holds in the same process, and the
//! binary is the thinnest thing that could parse a command line and call one. `show` is the one
//! errand over the directory alone: it reads what is on disk and asks the node nothing.

pub mod collect;
pub mod issue;
pub mod keys;
pub mod relate;
pub mod revoke;
pub mod show;

use almena_format::identifier::Did;
use almena_format::operation::{Operation, Signed};
use almena_suite::ed25519;

use crate::directory::{Directory, Keys};
use crate::failed::Failed;
use crate::node::Node;
use crate::post::envelope;
use crate::post::mediator;
use crate::post::message::Message;
use crate::relations::Relation;

/// The two things every errand needs: where this partner keeps itself, and which node it reads.
#[derive(Debug, Clone)]
pub struct Partner {
    /// The directory the keys and the memory are in.
    pub directory: Directory,
    /// The node every read goes through and every act is handed to.
    pub node: Node,
}

/// What one delivery of a message came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Delivered {
    /// How many of the far end's mediators took it.
    pub reached: usize,
    /// How many were asked.
    pub asked: usize,
}

impl Partner {
    /// The keys and the account, which every errand after `keys` needs.
    ///
    /// # Errors
    ///
    /// `partner_no_keys` and `partner_no_account`, which say to run `keys` first.
    pub fn identity(&self) -> Result<(Keys, Did), Failed> {
        let keys = self
            .directory
            .keys_held()?
            .ok_or_else(|| Failed::new("partner_no_keys"))?;
        let account = self
            .directory
            .account()?
            .ok_or_else(|| Failed::new("partner_no_account"))?;
        Ok((keys, account))
    }

    /// A mediator at a service a peer identifier names, pinned to the node that runs it.
    ///
    /// **Each mediator under its own key.** The identity travels beside the address inside the
    /// identifier, so a mediator on another node is verified against that node and never against
    /// the one this partner was told — a different machine with a different key.
    ///
    /// # Errors
    ///
    /// `node_not_an_origin`, or the peer's own refusals.
    pub fn mediator_at(&self, service: &(String, String)) -> Result<Node, Failed> {
        let (address, peer) = service;
        Node::at(address, peer)
    }

    /// Tell this partner's own mediator every address it answers to.
    ///
    /// # Errors
    ///
    /// What the mediator refused with.
    pub async fn declare(&self, keys: &Keys, account: &Did, epoch: u64) -> Result<(), Failed> {
        let relations = self.directory.relations()?;
        mediator::carry(
            &self.node,
            account,
            &keys.device_key()?,
            relations.addresses(),
            epoch,
        )
        .await
    }

    /// Seal a message for the far end of a relationship and hand it to every mediator it named.
    ///
    /// # Errors
    ///
    /// `send_nobody_yet` on a relationship nobody has answered, `send_not_sealed`, and
    /// `send_nowhere_to_send` when no mediator took it.
    pub async fn send(
        &self,
        relation: &Relation,
        kind: &str,
        id: &str,
        body: serde_json::Value,
    ) -> Result<Delivered, Failed> {
        let far = relation.far_end()?;
        let theirs = far.to_did();
        let message = Message::new(id, kind, &relation.mine, &theirs, body);
        let bytes = serde_json::to_vec(&message).map_err(|_| Failed::new("send_not_sealed"))?;
        let sealed = envelope::seal(&relation.key()?, &far.seals, &bytes)
            .map_err(|_| Failed::new("send_not_sealed"))?;
        let sealed = serde_json::to_vec(&sealed).map_err(|_| Failed::new("send_not_sealed"))?;

        let mut reached = 0;
        for service in &far.delivered_to {
            let (address, _) = service;
            let there = match self.mediator_at(service) {
                Ok(there) => there,
                Err(why) => {
                    log::info!("mediator_not_dialled service={address} reason={why}");
                    continue;
                }
            };
            match mediator::deliver(&there, &theirs, &sealed, mediator::HELD_FOR).await {
                Ok(()) => {
                    reached += 1;
                    log::info!("message_delivered kind={kind} mediator={address}");
                }
                Err(why) => {
                    log::info!("message_not_delivered kind={kind} mediator={address} reason={why}")
                }
            }
        }
        if reached == 0 {
            return Err(Failed::new("send_nowhere_to_send"));
        }
        Ok(Delivered {
            reached,
            asked: far.delivered_to.len(),
        })
    }
}

/// Sign an act with an Ed25519 key on behalf of that identifier.
pub fn signed_by(operation: &mut Operation, key: &ed25519::SigningKey, by: &Did) {
    let signature = key.sign(&operation.signing_bytes());
    operation.signatures.push(Signed {
        by: by.clone(),
        key: key.verifying_key().bytes().to_vec(),
        signature: signature.bytes(),
    });
}

/// Sixteen random bytes, written in base58, for a nonce or an identifier nobody else chose.
///
/// # Errors
///
/// `partner_no_entropy`.
pub fn drawn_name() -> Result<String, Failed> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|_| Failed::new("partner_no_entropy"))?;
    Ok(almena_format::identifier::base58(&bytes))
}
