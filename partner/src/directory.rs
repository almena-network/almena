//! The directory a partner keeps its keys and its memory in.
//!
//! **Named by the operator and nothing else.** A partner is an organisation's program, run on a
//! machine the organisation administers, and where it keeps what it keeps is that machine's
//! business: there is no platform directory here, no keyring and no passcode. What there is, is a
//! directory of small files with the permissions a key deserves, and the operator's own word for
//! where it is.
//!
//! # What is in it
//!
//! | File | What |
//! |---|---|
//! | `control.key` | The Ed25519 secret of the partner's own account, as hexadecimal |
//! | `device.key` | The P-256 secret of the one device that account has |
//! | `element.key` | The Ed25519 secret an issuer element is created with, which signs its status list acts |
//! | `issuance.key` | The P-256 secret that element emits credentials with |
//! | `account.json` | The account's identifier, once the node has taken it, and the issuer last issued as |
//! | `relations.json` | Every relationship, with the key of this end sealed by nothing |
//! | `issued.json` | Every credential issued, by identifier, and what the holder decided |
//! | `lists.json` | Every status list this partner publishes, by cohort |
//!
//! The keys are not sealed. A partner has no person to ask a passcode of, and a key sealed under
//! something written beside it is a key sealed by a promise; the directory's permissions are what
//! protect it, and that is said here so nobody expects more.
//!
//! # Why the element's keys are made here
//!
//! An issuer element is created in the record with a key, and whoever holds that key signs every
//! status list version the element publishes; the issuance key set on it signs every credential.
//! Both belong on the machine that issues — the one the operator administers — and never in a
//! browser tab or a wallet that composes the creation: those only need the public halves, which
//! `keys` prints for the operator to copy into the element form. Made once, like the account's.

use std::fs;
use std::path::{Path, PathBuf};

use almena_format::identifier::Did;
use almena_suite::ed25519;

use crate::failed::Failed;
use crate::issued::Issued;
use crate::lists::Lists;
use crate::relations::Relations;

/// What the control key is kept as.
const CONTROL: &str = "control.key";

/// What the device key is kept as.
const DEVICE: &str = "device.key";

/// What the issuer element's own key is kept as.
const ELEMENT: &str = "element.key";

/// What the issuance key is kept as.
const ISSUANCE: &str = "issuance.key";

/// What the account's identifier is kept as.
const ACCOUNT: &str = "account.json";

/// What the relationships are kept as.
const RELATIONS: &str = "relations.json";

/// What the credentials issued are kept as.
const ISSUED: &str = "issued.json";

/// What the status lists are kept as.
const LISTS: &str = "lists.json";

/// The two keys a partner is.
#[derive(Debug, Clone)]
pub struct Keys {
    /// The Ed25519 secret that governs the account.
    pub control: [u8; 32],
    /// The P-256 secret of the one device, which signs as an owner and collects post.
    pub device: [u8; 32],
}

impl Keys {
    /// The control key, ready to sign.
    #[must_use]
    pub fn control_key(&self) -> ed25519::SigningKey {
        ed25519::SigningKey::from_secret(self.control)
    }

    /// The device key, ready to sign.
    ///
    /// # Errors
    ///
    /// `keys_device_invalid` for a secret that is not a scalar on the curve, which a key this
    /// program drew cannot be and a file somebody edited can.
    pub fn device_key(&self) -> Result<almena_suite::p256::SigningKey, Failed> {
        almena_suite::p256::SigningKey::from_secret(self.device)
            .map_err(|_| Failed::new("keys_device_invalid"))
    }
}

/// The two keys an issuer element is, made on the issuing machine.
#[derive(Debug, Clone)]
pub struct ElementKeys {
    /// The Ed25519 secret the element is created with, which signs its status list acts.
    pub element: [u8; 32],
    /// The P-256 secret the element emits credentials with.
    pub issuance: [u8; 32],
}

impl ElementKeys {
    /// The element's own key, ready to sign.
    #[must_use]
    pub fn element_key(&self) -> ed25519::SigningKey {
        ed25519::SigningKey::from_secret(self.element)
    }

    /// The issuance key, ready to sign.
    ///
    /// # Errors
    ///
    /// `keys_issuance_invalid` for a secret that is not a scalar on the curve, which a key this
    /// program drew cannot be and a file somebody edited can.
    pub fn issuance_key(&self) -> Result<almena_suite::p256::SigningKey, Failed> {
        almena_suite::p256::SigningKey::from_secret(self.issuance)
            .map_err(|_| Failed::new("keys_issuance_invalid"))
    }
}

/// One partner's directory.
#[derive(Debug, Clone)]
pub struct Directory {
    path: PathBuf,
}

impl Directory {
    /// The directory at that path, made if it is not there.
    ///
    /// # Errors
    ///
    /// `directory_not_writable`.
    pub fn at(path: &Path) -> Result<Self, Failed> {
        fs::create_dir_all(path).map_err(|_| Failed::new("directory_not_writable"))?;
        Ok(Self {
            path: path.to_path_buf(),
        })
    }

    /// Where it is.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The keys, made if there are none yet.
    ///
    /// **Made once and read back every time after.** A partner that drew new keys on every run
    /// would be a new account on every run, and nothing it had issued would be its own any more.
    ///
    /// # Errors
    ///
    /// `keys_unreadable` for a file that is not a key, `keys_no_entropy` when the machine will not
    /// produce one, and `directory_not_writable`.
    pub fn keys(&self) -> Result<(Keys, bool), Failed> {
        if let Some(keys) = self.keys_held()? {
            return Ok((keys, false));
        }
        let keys = Keys {
            control: drawn()?,
            device: drawn_scalar()?,
        };
        write_secret(&self.path.join(CONTROL), &keys.control)?;
        write_secret(&self.path.join(DEVICE), &keys.device)?;
        Ok((keys, true))
    }

    /// The keys already in the directory, or nothing where there are none.
    ///
    /// # Errors
    ///
    /// `keys_unreadable`, and `keys_half_made` where one of the two is there and the other is not.
    pub fn keys_held(&self) -> Result<Option<Keys>, Failed> {
        let control = read_secret(&self.path.join(CONTROL))?;
        let device = read_secret(&self.path.join(DEVICE))?;
        match (control, device) {
            (Some(control), Some(device)) => Ok(Some(Keys { control, device })),
            (None, None) => Ok(None),
            _ => Err(Failed::new("keys_half_made")),
        }
    }

    /// The issuer element's keys, made if there are none yet.
    ///
    /// Made once and read back every time after, for the same reason the account's are: an
    /// element created with one key and signing with another is an element nobody controls.
    ///
    /// # Errors
    ///
    /// `keys_unreadable`, `keys_no_entropy`, `directory_not_writable`.
    pub fn element_keys(&self) -> Result<(ElementKeys, bool), Failed> {
        if let Some(keys) = self.element_keys_held()? {
            return Ok((keys, false));
        }
        let keys = ElementKeys {
            element: drawn()?,
            issuance: drawn_scalar()?,
        };
        write_secret(&self.path.join(ELEMENT), &keys.element)?;
        write_secret(&self.path.join(ISSUANCE), &keys.issuance)?;
        Ok((keys, true))
    }

    /// The element's keys already in the directory, or nothing where there are none.
    ///
    /// # Errors
    ///
    /// `keys_unreadable`, and `keys_half_made which=element` where one of the two is there and
    /// the other is not.
    pub fn element_keys_held(&self) -> Result<Option<ElementKeys>, Failed> {
        let element = read_secret(&self.path.join(ELEMENT))?;
        let issuance = read_secret(&self.path.join(ISSUANCE))?;
        match (element, issuance) {
            (Some(element), Some(issuance)) => Ok(Some(ElementKeys { element, issuance })),
            (None, None) => Ok(None),
            _ => Err(Failed::with("keys_half_made", "which", "element")),
        }
    }

    /// The element's own secret: the file the operator named, or `element.key` here.
    ///
    /// # Errors
    ///
    /// `keys_unreadable file=…` for a named file that is not there or not a key;
    /// `partner_no_element_keys` when nothing was named and `keys` has not made one.
    pub fn element_secret(&self, given: Option<&Path>) -> Result<[u8; 32], Failed> {
        self.secret_named(given, ELEMENT)
    }

    /// The issuance secret: the file the operator named, or `issuance.key` here.
    ///
    /// # Errors
    ///
    /// As [`Self::element_secret`].
    pub fn issuance_secret(&self, given: Option<&Path>) -> Result<[u8; 32], Failed> {
        self.secret_named(given, ISSUANCE)
    }

    /// A secret from the file named, or from this directory's file of that name.
    fn secret_named(&self, given: Option<&Path>, file: &str) -> Result<[u8; 32], Failed> {
        match given {
            Some(path) => read_secret(path)?.ok_or_else(|| {
                Failed::with("keys_unreadable", "file", &path.display().to_string())
            }),
            None => read_secret(&self.path.join(file))?
                .ok_or_else(|| Failed::with("partner_no_element_keys", "file", file)),
        }
    }

    /// The account's identifier, once the node has taken it.
    ///
    /// # Errors
    ///
    /// `account_unreadable`.
    pub fn account(&self) -> Result<Option<Did>, Failed> {
        let Some(held) = self.account_held()? else {
            return Ok(None);
        };
        Did::parse(&held.account)
            .map(Some)
            .map_err(|_| Failed::new("account_unreadable"))
    }

    /// Write the account's identifier down, keeping whatever else the file remembers.
    ///
    /// # Errors
    ///
    /// `directory_not_writable`.
    pub fn keep_account(&self, account: &Did) -> Result<(), Failed> {
        let held = Account {
            account: account.to_string(),
            issuer: self.account_held()?.and_then(|held| held.issuer),
        };
        write_json(&self.path.join(ACCOUNT), &held)
    }

    /// The issuer element `issue` was last asked to issue as, once it has been asked.
    ///
    /// # Errors
    ///
    /// `account_unreadable`.
    pub fn issuer(&self) -> Result<Option<Did>, Failed> {
        let Some(issuer) = self.account_held()?.and_then(|held| held.issuer) else {
            return Ok(None);
        };
        Did::parse(&issuer)
            .map(Some)
            .map_err(|_| Failed::new("account_unreadable"))
    }

    /// Remember the issuer element beside the account, so that `--issuer` is typed once.
    ///
    /// # Errors
    ///
    /// `partner_no_account` before `keys` has run; `directory_not_writable`.
    pub fn keep_issuer(&self, issuer: &Did) -> Result<(), Failed> {
        let mut held = self
            .account_held()?
            .ok_or_else(|| Failed::new("partner_no_account"))?;
        held.issuer = Some(issuer.to_string());
        write_json(&self.path.join(ACCOUNT), &held)
    }

    /// The account file, read, or nothing where there is none yet.
    fn account_held(&self) -> Result<Option<Account>, Failed> {
        let Some(text) = read_text(&self.path.join(ACCOUNT))? else {
            return Ok(None);
        };
        serde_json::from_str(&text)
            .map(Some)
            .map_err(|_| Failed::new("account_unreadable"))
    }

    /// Every relationship this partner has.
    ///
    /// # Errors
    ///
    /// `relations_unreadable`.
    pub fn relations(&self) -> Result<Relations, Failed> {
        read_json(&self.path.join(RELATIONS), "relations_unreadable")
    }

    /// Write the relationships back.
    ///
    /// # Errors
    ///
    /// `directory_not_writable`.
    pub fn keep_relations(&self, relations: &Relations) -> Result<(), Failed> {
        write_json(&self.path.join(RELATIONS), relations)
    }

    /// Every credential issued.
    ///
    /// # Errors
    ///
    /// `issued_unreadable`.
    pub fn issued(&self) -> Result<Issued, Failed> {
        read_json(&self.path.join(ISSUED), "issued_unreadable")
    }

    /// Write the credentials issued back.
    ///
    /// # Errors
    ///
    /// `directory_not_writable`.
    pub fn keep_issued(&self, issued: &Issued) -> Result<(), Failed> {
        write_json(&self.path.join(ISSUED), issued)
    }

    /// Every status list this partner publishes.
    ///
    /// # Errors
    ///
    /// `lists_unreadable`.
    pub fn lists(&self) -> Result<Lists, Failed> {
        read_json(&self.path.join(LISTS), "lists_unreadable")
    }

    /// Write the status lists back.
    ///
    /// # Errors
    ///
    /// `directory_not_writable`.
    pub fn keep_lists(&self, lists: &Lists) -> Result<(), Failed> {
        write_json(&self.path.join(LISTS), lists)
    }
}

/// The account's identifier, as it is kept, and the issuer element remembered beside it. A file
/// written before there was anything to remember has no `issuer`, and still reads.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct Account {
    account: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    issuer: Option<String>,
}

/// Thirty-two bytes from the operating system.
fn drawn() -> Result<[u8; 32], Failed> {
    let mut out = [0u8; 32];
    getrandom::fill(&mut out).map_err(|_| Failed::new("keys_no_entropy"))?;
    Ok(out)
}

/// Thirty-two bytes that are a P-256 scalar, drawn again in the vanishingly rare case they are not.
fn drawn_scalar() -> Result<[u8; 32], Failed> {
    for _ in 0..8 {
        let secret = drawn()?;
        if almena_suite::p256::SigningKey::from_secret(secret).is_ok() {
            return Ok(secret);
        }
    }
    Err(Failed::new("keys_no_entropy"))
}

/// A secret, read back from hexadecimal, or nothing where the file is not there.
///
/// # Errors
///
/// `keys_unreadable`.
pub fn read_secret(path: &Path) -> Result<Option<[u8; 32]>, Failed> {
    let Some(text) = read_text(path)? else {
        return Ok(None);
    };
    let bytes = unhex(text.trim()).ok_or_else(|| Failed::new("keys_unreadable"))?;
    let secret: [u8; 32] = bytes
        .try_into()
        .map_err(|_| Failed::new("keys_unreadable"))?;
    Ok(Some(secret))
}

/// A secret, written as hexadecimal and readable by its owner alone.
///
/// # Errors
///
/// `directory_not_writable`.
pub fn write_secret(path: &Path, secret: &[u8; 32]) -> Result<(), Failed> {
    fs::write(path, hex(secret)).map_err(|_| Failed::new("directory_not_writable"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))
            .map_err(|_| Failed::new("directory_not_writable"))?;
    }
    Ok(())
}

/// A file's text, or nothing where there is no file.
fn read_text(path: &Path) -> Result<Option<String>, Failed> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err(Failed::new("directory_not_readable")),
    }
}

/// A JSON file, or the empty value where there is no file yet.
fn read_json<T: serde::de::DeserializeOwned + Default>(
    path: &Path,
    why: &str,
) -> Result<T, Failed> {
    match read_text(path)? {
        Some(text) => serde_json::from_str(&text).map_err(|_| Failed::new(why)),
        None => Ok(T::default()),
    }
}

/// A JSON file, written whole and then moved into place, so that a crash leaves the old one.
fn write_json<T: serde::Serialize>(path: &Path, held: &T) -> Result<(), Failed> {
    let text =
        serde_json::to_string_pretty(held).map_err(|_| Failed::new("directory_not_writable"))?;
    let writing = path.with_extension("writing");
    fs::write(&writing, text).map_err(|_| Failed::new("directory_not_writable"))?;
    fs::rename(&writing, path).map_err(|_| Failed::new("directory_not_writable"))
}

/// Bytes as lower-case hexadecimal.
#[must_use]
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Bytes from hexadecimal, either case, or nothing for text that is not.
#[must_use]
pub fn unhex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) || !text.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&text[at..at + 2], 16).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{Directory, hex, unhex};

    fn scratch(name: &str) -> std::path::PathBuf {
        let path =
            std::env::temp_dir().join(format!("almena-partner-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        path
    }

    #[test]
    fn keys_are_made_once_and_read_back_every_time_after() {
        let path = scratch("keys");
        let directory = Directory::at(&path).expect("a directory");
        let (first, made) = directory.keys().expect("keys");
        assert!(made);
        let (again, made) = directory.keys().expect("keys");
        assert!(!made, "the second run is the same partner");
        assert_eq!(first.control, again.control);
        assert_eq!(first.device, again.device);
        assert!(again.device_key().is_ok());
        let _ = std::fs::remove_dir_all(&path);
    }

    #[cfg(unix)]
    #[test]
    fn a_key_is_readable_by_its_owner_alone() {
        use std::os::unix::fs::PermissionsExt as _;
        let path = scratch("mode");
        let directory = Directory::at(&path).expect("a directory");
        directory.keys().expect("keys");
        let mode = std::fs::metadata(path.join("control.key"))
            .expect("the file")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
        let _ = std::fs::remove_dir_all(&path);
    }

    #[test]
    fn a_directory_with_one_key_and_not_the_other_is_said_and_not_half_used() {
        let path = scratch("half");
        let directory = Directory::at(&path).expect("a directory");
        std::fs::write(path.join("control.key"), hex(&[7; 32])).expect("written");
        assert_eq!(directory.keys().unwrap_err().to_string(), "keys_half_made");
        let _ = std::fs::remove_dir_all(&path);
    }

    #[test]
    fn hexadecimal_goes_both_ways_and_odd_text_goes_nowhere() {
        assert_eq!(hex(&[0, 255, 16]), "00ff10");
        assert_eq!(unhex("00FF10"), Some(vec![0, 255, 16]));
        assert_eq!(unhex("0"), None);
        assert_eq!(unhex("zz"), None);
    }

    #[test]
    fn element_keys_are_made_once_beside_the_others_and_read_back_after() {
        let path = scratch("element");
        let directory = Directory::at(&path).expect("a directory");
        assert!(directory.element_keys_held().expect("read").is_none());
        let (first, made) = directory.element_keys().expect("element keys");
        assert!(made);
        let (again, made) = directory.element_keys().expect("element keys");
        assert!(!made, "the second run is the same issuer");
        assert_eq!(first.element, again.element);
        assert_eq!(first.issuance, again.issuance);
        assert_eq!(again.element_key().verifying_key().bytes().len(), 32);
        assert_eq!(
            again
                .issuance_key()
                .expect("a scalar")
                .verifying_key()
                .bytes()
                .len(),
            33
        );
        assert!(path.join("element.key").is_file());
        assert!(path.join("issuance.key").is_file());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            for file in ["element.key", "issuance.key"] {
                let mode = std::fs::metadata(path.join(file))
                    .expect("the file")
                    .permissions()
                    .mode();
                assert_eq!(mode & 0o777, 0o600, "{file}");
            }
        }
        let _ = std::fs::remove_dir_all(&path);
    }

    #[test]
    fn an_element_key_without_its_issuance_key_is_said_and_not_half_used() {
        let path = scratch("element-half");
        let directory = Directory::at(&path).expect("a directory");
        std::fs::write(path.join("issuance.key"), hex(&[7; 32])).expect("written");
        assert_eq!(
            directory.element_keys().unwrap_err().to_string(),
            "keys_half_made which=element"
        );
        let _ = std::fs::remove_dir_all(&path);
    }

    #[test]
    fn a_key_file_the_operator_names_wins_and_the_directory_s_own_is_the_default() {
        let path = scratch("default-keys");
        let directory = Directory::at(&path).expect("a directory");
        assert_eq!(
            directory.element_secret(None).unwrap_err().to_string(),
            "partner_no_element_keys file=element.key"
        );
        assert_eq!(
            directory.issuance_secret(None).unwrap_err().to_string(),
            "partner_no_element_keys file=issuance.key"
        );
        let (made, _) = directory.element_keys().expect("element keys");
        assert_eq!(directory.element_secret(None).expect("read"), made.element);
        assert_eq!(
            directory.issuance_secret(None).expect("read"),
            made.issuance
        );
        let elsewhere = path.join("other.key");
        std::fs::write(&elsewhere, hex(&[9; 32])).expect("written");
        assert_eq!(
            directory.element_secret(Some(&elsewhere)).expect("read"),
            [9; 32]
        );
        assert_eq!(
            directory.issuance_secret(Some(&elsewhere)).expect("read"),
            [9; 32]
        );
        let nowhere = path.join("nowhere.key");
        assert!(
            directory
                .issuance_secret(Some(&nowhere))
                .unwrap_err()
                .to_string()
                .starts_with("keys_unreadable file=")
        );
        let _ = std::fs::remove_dir_all(&path);
    }

    #[test]
    fn the_issuer_is_remembered_beside_the_account_and_an_older_file_still_reads() {
        use almena_format::identifier::Did;
        let path = scratch("issuer");
        let directory = Directory::at(&path).expect("a directory");
        let account = Did::parse("did:almena:dev:zQmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG")
            .expect("a did");
        let issuer = Did::parse("did:almena:dev:zQmZ56DfvnAoStjoSnF4jUK5LoZNE9T9k7z5nQGWvao1CRT")
            .expect("a did");
        assert_eq!(
            directory.keep_issuer(&issuer).unwrap_err().to_string(),
            "partner_no_account"
        );
        std::fs::write(
            path.join("account.json"),
            format!("{{\"account\": \"{account}\"}}"),
        )
        .expect("written");
        assert_eq!(directory.account().expect("read"), Some(account.clone()));
        assert_eq!(directory.issuer().expect("read"), None);
        directory.keep_issuer(&issuer).expect("kept");
        assert_eq!(directory.issuer().expect("read"), Some(issuer.clone()));
        assert_eq!(
            directory.account().expect("read"),
            Some(account.clone()),
            "the account is still there"
        );
        directory.keep_account(&account).expect("kept");
        assert_eq!(
            directory.issuer().expect("read"),
            Some(issuer),
            "writing the account again does not forget the issuer"
        );
        let _ = std::fs::remove_dir_all(&path);
    }
}
