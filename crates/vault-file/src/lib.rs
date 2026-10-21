//! Encrypted vault in a single file on the lawyer's disk (web app).
//!
//! The file is JSON:
//! - `vault_id`, a format version;
//! - `passphrase`: the DEK wrapped by Argon2id(passphrase);
//! - `recovery`: the DEK wrapped by the 24-word recovery phrase;
//! - `body`: the [`Store`] (clients, settings, audit, rules snapshots,
//!   watcher state) as JSON, encrypted with the DEK (AES-256-GCM).
//!
//! Unlocking needs the file and the passphrase; recovery (forgotten
//! passphrase) needs the file and the recovery phrase. The server hosting
//! the app never sees the file: the browser reads and writes it in place.
//!
//! Unlike the desktop vault there is no OS keychain, so the passphrase alone
//! protects a copied file: hence the Argon2id cost floor in `crypto`.

mod crypto;
mod store;

use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

pub use crypto::KdfParams;
use crypto::{PassphraseSlot, Secret, Wrapped};
pub use store::{
    ClientRecord, ClientSummary, DrawRow, Identity, ImportOutcome, ImportRow, SCHEMA, SnapshotRow,
    Store, WatchRun, WatchState,
};

/// Minimum passphrase length (characters).
pub const MIN_PASSPHRASE_CHARS: usize = 12;
const FORMAT: &str = "lawyertool-vault";
const VERSION: u32 = 1;
/// Largest vault file accepted (bytes).
pub const MAX_FILE: usize = 64 * 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("passphrase must be at least {MIN_PASSPHRASE_CHARS} characters")]
    WeakPassphrase,
    #[error("wrong passphrase")]
    WrongPassphrase,
    #[error("wrong recovery phrase")]
    WrongRecoveryPhrase,
    #[error("not a LawyerTool vault file")]
    NotAVault,
    #[error("vault format v{found} is newer than this app supports (v{supported}): reload the app")]
    NewerFormat { found: u32, supported: u32 },
    #[error("invalid input: {0}")]
    Invalid(&'static str),
    #[error("vault file is damaged: {0}")]
    Corrupt(&'static str),
    #[error("cryptography error: {0}")]
    Crypto(&'static str),
}

#[derive(Serialize, Deserialize)]
struct File {
    format: String,
    version: u32,
    vault_id: String,
    passphrase: PassphraseSlot,
    recovery: Wrapped,
    body: Wrapped,
}

/// Just enough of the file to check what it is before decrypting.
#[derive(Deserialize)]
struct Probe {
    format: String,
    version: u32,
}

/// The 24-word recovery phrase. Shown to the user once, at creation.
pub struct RecoveryPhrase(Zeroizing<String>);

impl RecoveryPhrase {
    fn from_secret(s: &Secret) -> Result<Self, Error> {
        let m = bip39::Mnemonic::from_entropy(s.expose())
            .map_err(|_| Error::Crypto("recovery phrase"))?;
        Ok(Self(Zeroizing::new(
            m.words().collect::<Vec<_>>().join(" "),
        )))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn parse_phrase(phrase: &str) -> Result<Secret, Error> {
    let normalized = Zeroizing::new(
        phrase
            .split_whitespace()
            .map(str::to_lowercase)
            .collect::<Vec<_>>()
            .join(" "),
    );
    let m = bip39::Mnemonic::parse_in(bip39::Language::English, normalized.as_str())
        .map_err(|_| Error::WrongRecoveryPhrase)?;
    let (entropy, len) = m.to_entropy_array();
    let entropy = Zeroizing::new(entropy);
    if len != crypto::KEY_LEN {
        return Err(Error::WrongRecoveryPhrase);
    }
    Secret::from_bytes(&entropy[..len])
}

fn check_passphrase(p: &str) -> Result<(), Error> {
    if p.chars().count() < MIN_PASSPHRASE_CHARS {
        Err(Error::WeakPassphrase)
    } else {
        Ok(())
    }
}

fn passphrase_slot(
    vault_id: &str,
    dek: &Secret,
    passphrase: &str,
    kdf: KdfParams,
) -> Result<PassphraseSlot, Error> {
    let salt = crypto::new_salt()?;
    let kek = crypto::passphrase_kek(passphrase, &salt, kdf)?;
    Ok(PassphraseSlot {
        version: 1,
        kdf,
        salt,
        wrapped: crypto::wrap(&kek, dek, vault_id, "passphrase")?,
    })
}

fn read(bytes: &[u8]) -> Result<File, Error> {
    if bytes.len() > MAX_FILE {
        return Err(Error::Corrupt("file too large"));
    }
    let probe: Probe = serde_json::from_slice(bytes).map_err(|_| Error::NotAVault)?;
    if probe.format != FORMAT {
        return Err(Error::NotAVault);
    }
    if probe.version > VERSION {
        return Err(Error::NewerFormat {
            found: probe.version,
            supported: VERSION,
        });
    }
    serde_json::from_slice(bytes).map_err(|_| Error::Corrupt("header"))
}

/// An unlocked vault: the decrypted store and the key to seal it again.
/// Dropping it wipes the key from memory.
pub struct Vault {
    store: Store,
    dek: Secret,
    vault_id: String,
    passphrase: PassphraseSlot,
    recovery: Wrapped,
}

impl std::fmt::Debug for Vault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Vault")
            .field("vault_id", &self.vault_id)
            .finish_non_exhaustive()
    }
}

impl std::ops::Deref for Vault {
    type Target = Store;
    fn deref(&self) -> &Store {
        &self.store
    }
}

impl std::ops::DerefMut for Vault {
    fn deref_mut(&mut self) -> &mut Store {
        &mut self.store
    }
}

impl Vault {
    /// Creates a new, empty vault. Returns it unlocked, with the recovery
    /// phrase to show the user once. Call [`Vault::seal`] to get the file.
    pub fn create(passphrase: &str, kdf: KdfParams) -> Result<(Self, RecoveryPhrase), Error> {
        check_passphrase(passphrase)?;
        let vault_id = crypto::new_vault_id()?;
        let dek = Secret::random()?;
        let recovery = Secret::random()?;
        let rkek = crypto::recovery_kek(&recovery, &vault_id)?;
        let vault = Self {
            store: Store::default(),
            passphrase: passphrase_slot(&vault_id, &dek, passphrase, kdf)?,
            recovery: crypto::wrap(&rkek, &dek, &vault_id, "recovery")?,
            dek,
            vault_id,
        };
        Ok((vault, RecoveryPhrase::from_secret(&recovery)?))
    }

    /// Unlocks a vault file with the passphrase.
    pub fn unlock(bytes: &[u8], passphrase: &str) -> Result<Self, Error> {
        let f = read(bytes)?;
        let kek = crypto::passphrase_kek(passphrase, &f.passphrase.salt, f.passphrase.kdf)?;
        let dek = crypto::unwrap(&kek, &f.passphrase.wrapped, &f.vault_id, "passphrase")
            .ok_or(Error::WrongPassphrase)?;
        Self::open_with(f, dek)
    }

    /// Unlocks with the recovery phrase and sets a new passphrase. The file
    /// must be sealed and saved again for the new passphrase to stick.
    pub fn recover(
        bytes: &[u8],
        phrase: &str,
        new_passphrase: &str,
        kdf: KdfParams,
    ) -> Result<Self, Error> {
        check_passphrase(new_passphrase)?;
        let f = read(bytes)?;
        let recovery = parse_phrase(phrase)?;
        let rkek = crypto::recovery_kek(&recovery, &f.vault_id)?;
        let dek = crypto::unwrap(&rkek, &f.recovery, &f.vault_id, "recovery")
            .ok_or(Error::WrongRecoveryPhrase)?;
        let mut v = Self::open_with(f, dek)?;
        v.passphrase = passphrase_slot(&v.vault_id, &v.dek, new_passphrase, kdf)?;
        Ok(v)
    }

    fn open_with(f: File, dek: Secret) -> Result<Self, Error> {
        let plain = crypto::open(&dek, &f.body, &f.vault_id).ok_or(Error::Corrupt("body"))?;
        let store: Store = serde_json::from_slice(&plain).map_err(|_| Error::Corrupt("body"))?;
        if store.schema > SCHEMA {
            return Err(Error::NewerFormat {
                found: store.schema,
                supported: SCHEMA,
            });
        }
        Ok(Self {
            store,
            dek,
            vault_id: f.vault_id,
            passphrase: f.passphrase,
            recovery: f.recovery,
        })
    }

    /// The vault file's bytes, freshly encrypted (new nonce every time).
    pub fn seal(&self) -> Result<Vec<u8>, Error> {
        let plain =
            Zeroizing::new(serde_json::to_vec(&self.store).map_err(|_| Error::Corrupt("body"))?);
        let f = File {
            format: FORMAT.into(),
            version: VERSION,
            vault_id: self.vault_id.clone(),
            passphrase: self.passphrase.clone(),
            recovery: self.recovery.clone(),
            body: crypto::seal(&self.dek, &plain, &self.vault_id)?,
        };
        serde_json::to_vec_pretty(&f).map_err(|_| Error::Corrupt("header"))
    }

    pub fn vault_id(&self) -> &str {
        &self.vault_id
    }

    pub fn change_passphrase(&mut self, new_passphrase: &str, kdf: KdfParams) -> Result<(), Error> {
        check_passphrase(new_passphrase)?;
        self.passphrase = passphrase_slot(&self.vault_id, &self.dek, new_passphrase, kdf)?;
        Ok(())
    }

    /// Wipes the key from memory.
    pub fn lock(self) {
        drop(self);
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    const FAST: KdfParams = KdfParams::INSECURE_FAST_FOR_TESTS;
    const PASS: &str = "a long test passphrase";

    fn identity() -> Identity {
        Identity {
            full_name: "Hélène Tremblay".into(),
            email: Some("h.t@example.com".into()),
            phone: None,
            file_reference: Some("2026-041".into()),
        }
    }

    #[test]
    fn create_seal_unlock_round_trip() {
        let (mut v, phrase) = Vault::create(PASS, FAST).unwrap();
        assert_eq!(phrase.as_str().split(' ').count(), 24);
        v.client_save(None, &identity(), "{}", "t").unwrap();
        v.meta_set("k", "v");
        let bytes = v.seal().unwrap();
        let text = String::from_utf8_lossy(&bytes);
        for leaked in ["Hélène", "Tremblay", "example.com", "2026-041"] {
            assert!(!text.contains(leaked), "{leaked} in clear");
        }
        let v2 = Vault::unlock(&bytes, PASS).unwrap();
        assert_eq!(v2.client_list().len(), 1);
        assert_eq!(v2.meta_get("k").as_deref(), Some("v"));
        assert_eq!(v2.vault_id(), v.vault_id());
    }

    #[test]
    fn wrong_passphrase_and_weak_passphrase() {
        let (v, _) = Vault::create(PASS, FAST).unwrap();
        let bytes = v.seal().unwrap();
        assert!(matches!(
            Vault::unlock(&bytes, "another long passphrase"),
            Err(Error::WrongPassphrase)
        ));
        assert!(matches!(
            Vault::create("short", FAST),
            Err(Error::WeakPassphrase)
        ));
    }

    #[test]
    fn recovery_sets_a_new_passphrase() {
        let (mut v, phrase) = Vault::create(PASS, FAST).unwrap();
        v.client_save(None, &identity(), "{}", "t").unwrap();
        let bytes = v.seal().unwrap();
        assert!(matches!(
            Vault::recover(&bytes, "abandon abandon", "new long passphrase", FAST),
            Err(Error::WrongRecoveryPhrase)
        ));
        // Case and spacing do not matter.
        let typed = format!("  {}  ", phrase.as_str().to_uppercase().replace(' ', "   "));
        let r = Vault::recover(&bytes, &typed, "new long passphrase", FAST).unwrap();
        let bytes = r.seal().unwrap();
        assert!(
            Vault::unlock(&bytes, PASS).is_err(),
            "old passphrase replaced"
        );
        let v = Vault::unlock(&bytes, "new long passphrase").unwrap();
        assert_eq!(v.client_list().len(), 1);
    }

    #[test]
    fn tampering_and_foreign_files_are_refused() {
        let (v, _) = Vault::create(PASS, FAST).unwrap();
        let bytes = v.seal().unwrap();
        let mut f: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let ct = f["body"]["ciphertext"].as_str().unwrap().to_owned();
        let flipped = if ct.starts_with('0') { "1" } else { "0" };
        f["body"]["ciphertext"] = format!("{flipped}{}", &ct[1..]).into();
        let bad = serde_json::to_vec(&f).unwrap();
        assert!(matches!(
            Vault::unlock(&bad, PASS),
            Err(Error::Corrupt("body"))
        ));

        // A body from another vault does not open under this vault's key.
        let (other, _) = Vault::create(PASS, FAST).unwrap();
        let mut o: serde_json::Value = serde_json::from_slice(&other.seal().unwrap()).unwrap();
        o["body"] = serde_json::from_slice::<serde_json::Value>(&bytes).unwrap()["body"].clone();
        assert!(Vault::unlock(&serde_json::to_vec(&o).unwrap(), PASS).is_err());

        assert!(matches!(
            Vault::unlock(b"%PDF-1.7", PASS),
            Err(Error::NotAVault)
        ));
        assert!(matches!(
            Vault::unlock(br#"{"format":"other","version":1}"#, PASS),
            Err(Error::NotAVault)
        ));
        let mut newer: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        newer["version"] = 99.into();
        assert!(matches!(
            Vault::unlock(&serde_json::to_vec(&newer).unwrap(), PASS),
            Err(Error::NewerFormat { found: 99, .. })
        ));
    }

    #[test]
    fn every_seal_uses_a_fresh_nonce() {
        let (v, _) = Vault::create(PASS, FAST).unwrap();
        assert_ne!(v.seal().unwrap(), v.seal().unwrap());
    }
}
