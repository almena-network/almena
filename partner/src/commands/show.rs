//! `show`: what the directory holds, printed so an operator can copy it.
//!
//! **Read from the directory and from nowhere else.** The account's identifier, the public halves
//! of the four keys, the issuer remembered and every relationship are all on disk; nothing here
//! asks the node, so `show` works without `--peer`, on a machine that is offline, and before the
//! account exists. What it prints is what a form elsewhere takes: the element's public key for
//! the registry's element form, the issuance key for `ISSUER_SET_ISSUANCE_KEY`, a relationship's
//! far end for `issue --to`.
//!
//! Each line is `key=value`, one value per line, and a key whose value is not held yet is printed
//! with nothing after the `=` — the same set of lines every time, so a shell can pick one out.

use almena_format::identifier::Did;

use crate::directory::{Directory, hex};
use crate::failed::Failed;

/// What the directory holds, as far as it is worth copying.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Shown {
    /// The account's identifier, once the node has taken it.
    pub account: Option<Did>,
    /// The device's public key, compressed, as hexadecimal.
    pub device: Option<String>,
    /// The issuer element's public key, the 32 Ed25519 bytes, as hexadecimal.
    pub element: Option<String>,
    /// The issuance public key, the 33 compressed P-256 bytes, as hexadecimal.
    pub issuance: Option<String>,
    /// The issuer element `issue` last issued as.
    pub issuer: Option<Did>,
    /// Every relationship: what this end is called, and what the far end is called once it has
    /// answered.
    pub relations: Vec<(String, Option<String>)>,
    /// Every credential issued: its identifier and the far end it was offered to.
    pub issued: Vec<(String, String)>,
}

impl Shown {
    /// The lines to print, in a fixed order.
    #[must_use]
    pub fn lines(&self) -> Vec<String> {
        let text = |held: Option<String>| held.unwrap_or_default();
        let mut lines = vec![
            format!(
                "account={}",
                text(self.account.as_ref().map(Did::to_string))
            ),
            format!("device={}", text(self.device.clone())),
            format!("element={}", text(self.element.clone())),
            format!("issuance={}", text(self.issuance.clone())),
            format!("issuer={}", text(self.issuer.as_ref().map(Did::to_string))),
        ];
        for (mine, theirs) in &self.relations {
            lines.push(format!(
                "relation mine={mine} theirs={}",
                text(theirs.clone())
            ));
        }
        for (identifier, relation) in &self.issued {
            lines.push(format!("credential={identifier} relation={relation}"));
        }
        lines
    }
}

/// Read the directory out.
///
/// # Errors
///
/// What the directory fails with: a file that is there and does not read.
pub fn run(directory: &Directory) -> Result<Shown, Failed> {
    let device = match directory.keys_held()? {
        Some(keys) => Some(hex(&keys.device_key()?.verifying_key().bytes())),
        None => None,
    };
    let (element, issuance) = match directory.element_keys_held()? {
        Some(keys) => (
            Some(hex(&keys.element_key().verifying_key().bytes())),
            Some(hex(&keys.issuance_key()?.verifying_key().bytes())),
        ),
        None => (None, None),
    };
    Ok(Shown {
        account: directory.account()?,
        device,
        element,
        issuance,
        issuer: directory.issuer()?,
        relations: directory
            .relations()?
            .all()
            .into_iter()
            .map(|relation| (relation.mine.clone(), relation.theirs.clone()))
            .collect(),
        issued: directory
            .issued()?
            .all()
            .into_iter()
            .map(|(identifier, record)| (identifier.clone(), record.relation.clone()))
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::{Shown, run};
    use crate::directory::Directory;
    use crate::issued::Record;
    use crate::relations::Relations;
    use almena_format::identifier::Did;

    fn scratch(name: &str) -> std::path::PathBuf {
        let path =
            std::env::temp_dir().join(format!("almena-partner-show-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        path
    }

    #[test]
    fn an_empty_directory_shows_every_key_with_nothing_after_it() {
        let path = scratch("empty");
        let directory = Directory::at(&path).expect("a directory");
        let shown = run(&directory).expect("shown");
        assert_eq!(shown, Shown::default());
        assert_eq!(
            shown.lines(),
            vec!["account=", "device=", "element=", "issuance=", "issuer="]
        );
        let _ = std::fs::remove_dir_all(&path);
    }

    /// A directory with one relationship and one credential written down, as `relate` and
    /// `issue` leave them.
    fn with_a_relationship_and_a_credential(directory: &Directory) {
        let mut relations = Relations::default();
        relations.keep(crate::relations::Relation {
            mine: "did:peer:2.mine".to_owned(),
            theirs: Some("did:peer:2.theirs".to_owned()),
            secret: String::new(),
        });
        directory.keep_relations(&relations).expect("kept");
        let mut issued = crate::issued::Issued::default();
        issued.keep(
            "one-degree",
            Record {
                written: String::new(),
                relation: "did:peer:2.theirs".to_owned(),
                list: None,
                index: None,
                decided: None,
                revoked_at: None,
            },
        );
        directory.keep_issued(&issued).expect("kept");
    }

    #[test]
    fn what_keys_made_and_issue_remembered_is_shown_as_it_is_copied() {
        let path = scratch("held");
        let directory = Directory::at(&path).expect("a directory");
        let (keys, _) = directory.keys().expect("keys");
        let (element, _) = directory.element_keys().expect("element keys");
        let account = Did::parse("did:almena:dev:zQmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG")
            .expect("a did");
        let issuer = Did::parse("did:almena:dev:zQmZ56DfvnAoStjoSnF4jUK5LoZNE9T9k7z5nQGWvao1CRT")
            .expect("a did");
        directory.keep_account(&account).expect("kept");
        directory.keep_issuer(&issuer).expect("kept");
        with_a_relationship_and_a_credential(&directory);

        let shown = run(&directory).expect("shown");
        let lines = shown.lines();
        assert_eq!(lines[0], format!("account={account}"));
        assert_eq!(
            lines[1],
            format!(
                "device={}",
                crate::directory::hex(&keys.device_key().expect("a key").verifying_key().bytes())
            )
        );
        assert_eq!(
            lines[2],
            format!(
                "element={}",
                crate::directory::hex(&element.element_key().verifying_key().bytes())
            )
        );
        assert_eq!(
            lines[2].len(),
            "element=".len() + 64,
            "32 bytes, hexadecimal"
        );
        assert_eq!(
            lines[3].len(),
            "issuance=".len() + 66,
            "33 bytes, hexadecimal"
        );
        assert_eq!(lines[4], format!("issuer={issuer}"));
        assert_eq!(
            lines[5],
            "relation mine=did:peer:2.mine theirs=did:peer:2.theirs"
        );
        assert_eq!(lines[6], "credential=one-degree relation=did:peer:2.theirs");
        assert_eq!(lines.len(), 7);
        let _ = std::fs::remove_dir_all(&path);
    }
}
