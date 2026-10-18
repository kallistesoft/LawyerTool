//! Where the passphrase-wrapped key lives: the OS keychain in the app, an
//! in-memory map in tests.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use crate::Error;

/// Keychain service name; the account is the vault id.
pub const SERVICE: &str = "ca.lawyertool.desktop";

pub trait KeyStore {
    fn get(&self, account: &str) -> Result<Option<Vec<u8>>, Error>;
    fn set(&self, account: &str, value: &[u8]) -> Result<(), Error>;
    fn delete(&self, account: &str) -> Result<(), Error>;
}

/// macOS login keychain / Windows Credential Manager.
pub struct OsKeyStore;

fn init_store() -> Result<(), String> {
    static INIT: OnceLock<Result<(), String>> = OnceLock::new();
    INIT.get_or_init(|| {
        #[cfg(target_os = "macos")]
        let store: Arc<keyring_core::CredentialStore> =
            apple_native_keyring_store::keychain::Store::new().map_err(|e| e.to_string())?;
        #[cfg(windows)]
        let store: Arc<keyring_core::CredentialStore> =
            windows_native_keyring_store::Store::new().map_err(|e| e.to_string())?;
        #[cfg(not(any(target_os = "macos", windows)))]
        return Err("no supported OS keychain on this platform".to_owned());
        #[cfg(any(target_os = "macos", windows))]
        {
            keyring_core::set_default_store(store);
            Ok(())
        }
    })
    .clone()
}

impl OsKeyStore {
    fn entry(account: &str) -> Result<keyring_core::Entry, Error> {
        init_store().map_err(Error::Keychain)?;
        keyring_core::Entry::new(SERVICE, account).map_err(|e| Error::Keychain(e.to_string()))
    }
}

impl KeyStore for OsKeyStore {
    fn get(&self, account: &str) -> Result<Option<Vec<u8>>, Error> {
        match Self::entry(account)?.get_secret() {
            Ok(v) => Ok(Some(v)),
            Err(keyring_core::Error::NoEntry) => Ok(None),
            Err(e) => Err(Error::Keychain(e.to_string())),
        }
    }

    fn set(&self, account: &str, value: &[u8]) -> Result<(), Error> {
        Self::entry(account)?
            .set_secret(value)
            .map_err(|e| Error::Keychain(e.to_string()))
    }

    fn delete(&self, account: &str) -> Result<(), Error> {
        match Self::entry(account)?.delete_credential() {
            Ok(()) | Err(keyring_core::Error::NoEntry) => Ok(()),
            Err(e) => Err(Error::Keychain(e.to_string())),
        }
    }
}

/// In-memory store for tests and tooling.
#[derive(Default)]
pub struct MemoryKeyStore(Mutex<HashMap<String, Vec<u8>>>);

impl KeyStore for MemoryKeyStore {
    fn get(&self, account: &str) -> Result<Option<Vec<u8>>, Error> {
        Ok(self
            .0
            .lock()
            .map_err(|_| Error::Keychain("poisoned".into()))?
            .get(account)
            .cloned())
    }

    fn set(&self, account: &str, value: &[u8]) -> Result<(), Error> {
        self.0
            .lock()
            .map_err(|_| Error::Keychain("poisoned".into()))?
            .insert(account.to_owned(), value.to_vec());
        Ok(())
    }

    fn delete(&self, account: &str) -> Result<(), Error> {
        self.0
            .lock()
            .map_err(|_| Error::Keychain("poisoned".into()))?
            .remove(account);
        Ok(())
    }
}
