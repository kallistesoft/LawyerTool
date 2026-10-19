//! Development builds only (`cfg(debug_assertions)`; compiled out of
//! release builds, enforced by scripts/check-deps.sh).
//!
//! Opens a separate test vault automatically, so `tauri dev` needs no
//! passphrase: the key is wrapped with a fixed dev passphrase and kept in a
//! plain file next to the dev database instead of the OS keychain. Never use
//! it with real client data. Set `LAWYERTOOL_REAL_VAULT=1` to test the real
//! passphrase flow in a dev build.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use vault::{Error, KdfParams, KeyStore, Paths, Vault};

const DEV_PASSPHRASE: &str = "dev-only-not-a-secret";

pub fn enabled() -> bool {
    std::env::var_os("LAWYERTOOL_REAL_VAULT").is_none()
}

/// Keystore in a JSON file (hex values). Dev only: not protected.
struct FileKeyStore(PathBuf);

impl FileKeyStore {
    fn load(&self) -> BTreeMap<String, String> {
        std::fs::read(&self.0)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    fn save(&self, m: &BTreeMap<String, String>) -> Result<(), Error> {
        let json = serde_json::to_vec_pretty(m).map_err(|_| Error::Corrupt("dev keystore"))?;
        Ok(std::fs::write(&self.0, json)?)
    }
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn unhex(s: &str) -> Option<Vec<u8>> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok())
        .collect()
}

impl KeyStore for FileKeyStore {
    fn get(&self, account: &str) -> Result<Option<Vec<u8>>, Error> {
        Ok(self.load().get(account).and_then(|h| unhex(h)))
    }

    fn set(&self, account: &str, value: &[u8]) -> Result<(), Error> {
        let mut m = self.load();
        m.insert(account.to_owned(), hex(value));
        self.save(&m)
    }

    fn delete(&self, account: &str) -> Result<(), Error> {
        let mut m = self.load();
        m.remove(account);
        self.save(&m)
    }
}

pub fn db_path(dir: &Path) -> PathBuf {
    dir.join("lawyertool-dev.db")
}

/// Creates the dev vault on first run, then unlocks it.
pub fn open(dir: &Path) -> Result<Vault, Error> {
    std::fs::create_dir_all(dir)?;
    let paths = Paths::new(db_path(dir));
    let store = FileKeyStore(dir.join("dev-keystore.json"));
    if paths.exists() {
        match Vault::unlock(&paths, &store, DEV_PASSPHRASE) {
            // Key file emptied by the v0.1.6-v0.1.11 migration bug: rebuild
            // it from the dev keystore (single vault).
            Err(Error::Corrupt("key file")) => {
                let ids: Vec<String> = store.load().into_keys().collect();
                match ids.as_slice() {
                    [id] => {
                        Vault::rebuild_keyfile(&paths, &store, id, DEV_PASSPHRASE).map(|(v, _)| v)
                    }
                    _ => Err(Error::Corrupt("key file")),
                }
            }
            other => other,
        }
    } else {
        // The recovery phrase of a dev vault is not needed: discard it.
        Vault::create(
            &paths,
            &store,
            DEV_PASSPHRASE,
            KdfParams::INSECURE_FAST_FOR_TESTS,
        )
        .map(|(v, _)| v)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    #[test]
    fn dev_vault_reopens_without_passphrase() {
        let dir = std::env::temp_dir().join(format!("lt-dev-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let v = super::open(&dir).unwrap();
        v.conn()
            .execute("INSERT INTO meta (key, value) VALUES ('k', 'v')", [])
            .unwrap();
        v.lock();
        let v = super::open(&dir).unwrap();
        let got: String = v
            .conn()
            .query_row("SELECT value FROM meta WHERE key = 'k'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(got, "v");
        drop(v);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
