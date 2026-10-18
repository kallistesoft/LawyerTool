//! Encrypted local storage.
//!
//! Files, side by side:
//! - `<name>.db`: SQLCipher database (AES-256), keyed with a random DEK;
//! - `<name>.keys`: vault id + the DEK wrapped by the recovery phrase.
//!
//! The DEK wrapped by the passphrase (Argon2id) lives in the OS keychain.
//! Unlocking needs the keychain entry and the passphrase; recovery (new
//! laptop, lost keychain) needs the `.db`, the `.keys` file and the 24-word
//! recovery phrase. Copying the files alone, or knowing the passphrase alone,
//! is not enough.

mod clients;
mod crypto;
mod db;
mod keystore;

use std::path::{Path, PathBuf};

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

pub use clients::{ClientRecord, ClientSummary, Identity};
pub use crypto::KdfParams;
use crypto::{PassphraseSlot, Secret, Wrapped};
pub use db::schema_version;
pub use keystore::{KeyStore, MemoryKeyStore, OsKeyStore};

/// Minimum passphrase length (characters).
pub const MIN_PASSPHRASE_CHARS: usize = 12;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("passphrase must be at least {MIN_PASSPHRASE_CHARS} characters")]
    WeakPassphrase,
    #[error("wrong passphrase")]
    WrongPassphrase,
    #[error("wrong recovery phrase")]
    WrongRecoveryPhrase,
    #[error("the database key does not match this database")]
    WrongKey,
    #[error("no vault at {0}")]
    NotFound(PathBuf),
    #[error("{0} already exists")]
    AlreadyExists(PathBuf),
    #[error("this computer's keychain has no key for this vault: use the recovery phrase")]
    KeychainEntryMissing,
    #[error(
        "database schema v{found} is newer than this app supports (v{supported}): update the app"
    )]
    NewerSchema { found: u32, supported: u32 },
    #[error("invalid input: {0}")]
    Invalid(&'static str),
    #[error("vault files are damaged: {0}")]
    Corrupt(&'static str),
    #[error("keychain error: {0}")]
    Keychain(String),
    #[error("cryptography error: {0}")]
    Crypto(&'static str),
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("file error: {0}")]
    Io(#[from] std::io::Error),
}

/// The vault's file locations.
#[derive(Debug, Clone)]
pub struct Paths {
    pub db: PathBuf,
}

impl Paths {
    pub fn new(db: impl Into<PathBuf>) -> Self {
        Self { db: db.into() }
    }

    pub fn keys(&self) -> PathBuf {
        self.db.with_extension("keys")
    }

    pub fn exists(&self) -> bool {
        self.db.exists() && self.keys().exists()
    }
}

#[derive(Serialize, Deserialize)]
struct KeyFile {
    version: u32,
    vault_id: String,
    recovery: Wrapped,
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

fn enum_of<T: serde::de::DeserializeOwned>(s: String) -> Result<T, serde_json::Error> {
    serde_json::from_value(serde_json::Value::String(s))
}

fn check_passphrase(p: &str) -> Result<(), Error> {
    if p.chars().count() < MIN_PASSPHRASE_CHARS {
        Err(Error::WeakPassphrase)
    } else {
        Ok(())
    }
}

fn read_keyfile(paths: &Paths) -> Result<KeyFile, Error> {
    if !paths.exists() {
        return Err(Error::NotFound(paths.db.clone()));
    }
    serde_json::from_slice(&std::fs::read(paths.keys())?).map_err(|_| Error::Corrupt("key file"))
}

fn store_passphrase_slot(
    store: &dyn KeyStore,
    vault_id: &str,
    dek: &Secret,
    passphrase: &str,
    kdf: KdfParams,
) -> Result<(), Error> {
    let salt = crypto::new_salt()?;
    let kek = crypto::passphrase_kek(passphrase, &salt, kdf)?;
    let slot = PassphraseSlot {
        version: 1,
        kdf,
        salt,
        wrapped: crypto::wrap(&kek, dek, vault_id, "passphrase")?,
    };
    store.set(
        vault_id,
        &serde_json::to_vec(&slot).map_err(|_| Error::Corrupt("passphrase slot"))?,
    )
}

/// An unlocked vault. Dropping it (or [`Vault::lock`]) closes the database
/// and wipes the key from memory.
pub struct Vault {
    conn: Connection,
    dek: Secret,
    vault_id: String,
    paths: Paths,
    /// Backup taken before the last migration, if any.
    pub migration_backup: Option<PathBuf>,
}

impl std::fmt::Debug for Vault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Vault")
            .field("db", &self.paths.db)
            .finish_non_exhaustive()
    }
}

impl Vault {
    /// Creates a new vault. Returns it unlocked, with the recovery phrase to
    /// show the user once.
    pub fn create(
        paths: &Paths,
        store: &dyn KeyStore,
        passphrase: &str,
        kdf: KdfParams,
    ) -> Result<(Self, RecoveryPhrase), Error> {
        check_passphrase(passphrase)?;
        for p in [&paths.db, &paths.keys()] {
            if p.exists() {
                return Err(Error::AlreadyExists(p.clone()));
            }
        }
        let vault_id = crypto::new_vault_id()?;
        let dek = Secret::random()?;
        let recovery = Secret::random()?;
        let rkek = crypto::recovery_kek(&recovery, &vault_id)?;
        let keyfile = KeyFile {
            version: 1,
            vault_id: vault_id.clone(),
            recovery: crypto::wrap(&rkek, &dek, &vault_id, "recovery")?,
        };
        std::fs::write(
            paths.keys(),
            serde_json::to_vec_pretty(&keyfile).map_err(|_| Error::Corrupt("key file"))?,
        )?;
        store_passphrase_slot(store, &vault_id, &dek, passphrase, kdf)?;
        let phrase = RecoveryPhrase::from_secret(&recovery)?;
        let vault = Self::open_with(paths, dek, vault_id)?;
        Ok((vault, phrase))
    }

    /// Unlocks with the passphrase (keychain entry required).
    pub fn unlock(paths: &Paths, store: &dyn KeyStore, passphrase: &str) -> Result<Self, Error> {
        let keyfile = read_keyfile(paths)?;
        let raw = store
            .get(&keyfile.vault_id)?
            .ok_or(Error::KeychainEntryMissing)?;
        let slot: PassphraseSlot =
            serde_json::from_slice(&raw).map_err(|_| Error::Corrupt("keychain entry"))?;
        let kek = crypto::passphrase_kek(passphrase, &slot.salt, slot.kdf)?;
        let dek = crypto::unwrap(&kek, &slot.wrapped, &keyfile.vault_id, "passphrase")
            .ok_or(Error::WrongPassphrase)?;
        Self::open_with(paths, dek, keyfile.vault_id)
    }

    /// Unlocks with the recovery phrase and sets a new passphrase in this
    /// computer's keychain (new laptop, lost keychain, forgotten passphrase).
    pub fn recover(
        paths: &Paths,
        store: &dyn KeyStore,
        phrase: &str,
        new_passphrase: &str,
        kdf: KdfParams,
    ) -> Result<Self, Error> {
        check_passphrase(new_passphrase)?;
        let keyfile = read_keyfile(paths)?;
        let recovery = parse_phrase(phrase)?;
        let rkek = crypto::recovery_kek(&recovery, &keyfile.vault_id)?;
        let dek = crypto::unwrap(&rkek, &keyfile.recovery, &keyfile.vault_id, "recovery")
            .ok_or(Error::WrongRecoveryPhrase)?;
        let vault = Self::open_with(paths, dek, keyfile.vault_id)?;
        store_passphrase_slot(store, &vault.vault_id, &vault.dek, new_passphrase, kdf)?;
        Ok(vault)
    }

    fn open_with(paths: &Paths, dek: Secret, vault_id: String) -> Result<Self, Error> {
        let mut conn = db::open(&paths.db, &dek)?;
        let migration_backup = db::migrate(&mut conn, &paths.db, &dek)?;
        if let Some(b) = &migration_backup {
            std::fs::copy(paths.keys(), b.with_extension("keys"))?;
        }
        Ok(Self {
            conn,
            dek,
            vault_id,
            paths: paths.clone(),
            migration_backup,
        })
    }

    pub fn change_passphrase(
        &self,
        store: &dyn KeyStore,
        new_passphrase: &str,
        kdf: KdfParams,
    ) -> Result<(), Error> {
        check_passphrase(new_passphrase)?;
        store_passphrase_slot(store, &self.vault_id, &self.dek, new_passphrase, kdf)
    }

    /// Encrypted backup: `<dest>` (database, same key) and `<dest>.keys`.
    /// Restoring needs both files and the recovery phrase.
    pub fn backup(&self, dest: &Path) -> Result<(), Error> {
        let keys = dest.with_extension("keys");
        if keys.exists() {
            return Err(Error::AlreadyExists(keys));
        }
        db::export(&self.conn, dest, &self.dek)?;
        std::fs::copy(self.paths.keys(), keys)?;
        Ok(())
    }

    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    pub fn conn_mut(&mut self) -> &mut Connection {
        &mut self.conn
    }

    pub fn schema_version(&self) -> Result<u32, Error> {
        Ok(self
            .conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))?)
    }

    /// Appends an audit record (see the `audit` crate: metadata only).
    pub fn audit_record(&self, r: &audit::AuditRecord) -> Result<(), Error> {
        let tag = |v: serde_json::Value| {
            v.as_str()
                .map(str::to_owned)
                .ok_or(Error::Corrupt("audit enum"))
        };
        let category =
            tag(serde_json::to_value(r.category).map_err(|_| Error::Corrupt("audit enum"))?)?;
        let outcome =
            tag(serde_json::to_value(r.outcome).map_err(|_| Error::Corrupt("audit enum"))?)?;
        self.conn.execute(
            "INSERT INTO audit_log (at, method, host, category, source_file, outcome, status, bytes_sent, bytes_received, reason)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            rusqlite::params![
                r.at,
                r.method,
                r.host,
                category,
                r.source_file,
                outcome,
                r.status,
                i64::try_from(r.bytes_sent).unwrap_or(i64::MAX),
                i64::try_from(r.bytes_received).unwrap_or(i64::MAX),
                r.reason
            ],
        )?;
        Ok(())
    }

    /// Most recent audit records first.
    pub fn audit_list(&self, limit: u32) -> Result<Vec<audit::AuditRecord>, Error> {
        let mut st = self.conn.prepare(
            "SELECT at, method, host, category, source_file, outcome, status, bytes_sent, bytes_received, reason
             FROM audit_log ORDER BY id DESC LIMIT ?1",
        )?;
        let rows = st.query_map([limit], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, Option<u16>>(6)?,
                row.get::<_, i64>(7)?,
                row.get::<_, i64>(8)?,
                row.get::<_, Option<String>>(9)?,
            ))
        })?;
        rows.map(|r| {
            let (at, method, host, category, source_file, outcome, status, sent, received, reason) =
                r?;
            Ok(audit::AuditRecord {
                at,
                method,
                host,
                category: enum_of::<audit::DataCategory>(category)
                    .map_err(|_| Error::Corrupt("audit category"))?,
                source_file,
                outcome: enum_of::<audit::Outcome>(outcome)
                    .map_err(|_| Error::Corrupt("audit outcome"))?,
                status,
                bytes_sent: u64::try_from(sent).unwrap_or(0),
                bytes_received: u64::try_from(received).unwrap_or(0),
                reason,
            })
        })
        .collect()
    }

    /// Closes the database and wipes the key from memory.
    pub fn lock(self) {
        drop(self);
    }
}
