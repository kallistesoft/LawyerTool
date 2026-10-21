//! The web app's engine. Compiled to WebAssembly and run in a Web Worker;
//! the page talks to it through `call(command, args)`, like the desktop
//! app's Tauri commands, so the Svelte UI barely changes.
//!
//! What lives here: the unlocked vault (in memory), the rules engine,
//! de-identification, report rendering, inbox bookkeeping and the review of
//! the watcher's published snapshot. What the page does: reading and writing
//! the vault file and the chosen folders (File System Access API), fetching
//! `watch/latest.json` from its own origin, and the idle timer.
//!
//! Client data never leaves the lawyer's computer: this crate has no network
//! access, and the page only ever fetches public files from its own origin.

mod clients;
mod export;
mod inbox;
mod watch;

#[cfg(target_arch = "wasm32")]
mod bindings;

use serde::Serialize;
use vault_file::{KdfParams, Vault};
use zeroize::Zeroizing;

/// Error sent to the page: a stable code and a user-facing message (French).
/// Never contains passphrases, keys or client data.
#[derive(Debug, Serialize)]
pub struct CommandError {
    code: &'static str,
    message: String,
}

impl CommandError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn internal() -> Self {
        Self {
            code: "internal",
            message: "Erreur interne.".into(),
        }
    }
}

impl From<vault_file::Error> for CommandError {
    fn from(e: vault_file::Error) -> Self {
        use vault_file::Error::*;
        let (code, message) = match &e {
            WeakPassphrase => (
                "weak_passphrase",
                format!(
                    "La phrase de passe doit contenir au moins {} caractères.",
                    vault_file::MIN_PASSPHRASE_CHARS
                ),
            ),
            WrongPassphrase => ("wrong_passphrase", "Phrase de passe incorrecte.".into()),
            WrongRecoveryPhrase => (
                "wrong_recovery_phrase",
                "Phrase de récupération incorrecte.".into(),
            ),
            NotAVault => (
                "not_a_vault",
                "Ce fichier n’est pas un coffre LawyerTool.".into(),
            ),
            NewerFormat { .. } => (
                "newer_format",
                "Ce coffre a été enregistré par une version plus récente : rechargez la page."
                    .into(),
            ),
            Invalid(what) => ("invalid", format!("Donnée invalide : {what}.")),
            Corrupt(_) => ("corrupt", "Le fichier du coffre est endommagé.".into()),
            Crypto(_) => ("vault_error", "Le coffre n'a pas pu être ouvert.".into()),
        };
        Self { code, message }
    }
}

pub type Res<T> = Result<T, CommandError>;

pub fn err(code: &'static str, message: impl Into<String>) -> CommandError {
    CommandError::new(code, message)
}

/// Clock. `SystemTime::now` panics in the browser, so the wasm build asks
/// JavaScript.
pub mod time {
    /// Seconds since the Unix epoch.
    pub fn now_secs() -> u64 {
        #[cfg(target_arch = "wasm32")]
        {
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let s = (js_sys::Date::now() / 1000.0) as u64;
            s
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_secs())
        }
    }

    /// Current UTC time as RFC 3339.
    pub fn now() -> String {
        audit::rfc3339(now_secs())
    }

    /// RFC 3339 time `secs` seconds ago.
    pub fn ago(secs: u64) -> String {
        audit::rfc3339(now_secs().saturating_sub(secs))
    }

    /// Today's date in UTC.
    pub fn today() -> applicant::IsoDate {
        let days = i32::try_from(now_secs() / 86_400).unwrap_or(0);
        applicant::IsoDate::from_days(days).unwrap_or_else(|| {
            applicant::IsoDate::parse("1970-01-01").unwrap_or_else(|| unreachable!())
        })
    }
}

#[derive(Serialize)]
pub struct Created {
    recovery_phrase: String,
}

/// The engine's state: the unlocked vault, if any, and whether it changed
/// since the page last saved it.
pub struct App {
    vault: Option<Vault>,
    dirty: bool,
    kdf: KdfParams,
}

impl Default for App {
    fn default() -> Self {
        Self::new(KdfParams::DEFAULT)
    }
}

impl App {
    pub fn new(kdf: KdfParams) -> Self {
        Self {
            vault: None,
            dirty: false,
            kdf,
        }
    }

    pub fn is_unlocked(&self) -> bool {
        self.vault.is_some()
    }

    fn vault(&self) -> Res<&Vault> {
        self.vault
            .as_ref()
            .ok_or_else(|| err("locked", "Le coffre est verrouillé."))
    }

    /// Mutable access marks the vault as needing a save.
    fn vault_mut(&mut self) -> Res<&mut Vault> {
        let v = self
            .vault
            .as_mut()
            .ok_or_else(|| err("locked", "Le coffre est verrouillé."))?;
        self.dirty = true;
        Ok(v)
    }

    /// Creates a new vault (the page then saves [`App::seal`] to a new file).
    pub fn create(&mut self, passphrase: &str) -> Res<Created> {
        let passphrase = Zeroizing::new(passphrase.to_owned());
        let (v, phrase) = Vault::create(&passphrase, self.kdf)?;
        self.vault = Some(v);
        self.dirty = true;
        // Shown once by the page, never stored.
        Ok(Created {
            recovery_phrase: phrase.as_str().to_owned(),
        })
    }

    pub fn unlock(&mut self, file: &[u8], passphrase: &str) -> Res<()> {
        let passphrase = Zeroizing::new(passphrase.to_owned());
        self.vault = Some(Vault::unlock(file, &passphrase)?);
        self.dirty = false;
        Ok(())
    }

    /// Unlocks with the recovery phrase; the new passphrase applies once saved.
    pub fn recover(&mut self, file: &[u8], phrase: &str, new_passphrase: &str) -> Res<()> {
        let (phrase, new_passphrase) = (
            Zeroizing::new(phrase.to_owned()),
            Zeroizing::new(new_passphrase.to_owned()),
        );
        self.vault = Some(Vault::recover(file, &phrase, &new_passphrase, self.kdf)?);
        self.dirty = true;
        Ok(())
    }

    /// Drops the vault (wipes the key). Returns whether it was open.
    pub fn lock(&mut self) -> bool {
        self.dirty = false;
        self.vault.take().is_some()
    }

    pub fn vault_id(&self) -> Option<String> {
        self.vault.as_ref().map(|v| v.vault_id().to_owned())
    }

    /// Whether the vault changed since the last call (the page saves then).
    pub fn take_dirty(&mut self) -> bool {
        std::mem::take(&mut self.dirty)
    }

    /// The vault file's bytes.
    pub fn seal(&self) -> Res<Vec<u8>> {
        Ok(self.vault()?.seal()?)
    }

    /// Runs one JSON command (see `bindings.rs` for the page side).
    pub fn call(&mut self, cmd: &str, args: &serde_json::Value) -> Res<serde_json::Value> {
        fn arg<T: serde::de::DeserializeOwned>(args: &serde_json::Value, key: &str) -> Res<T> {
            serde_json::from_value(args.get(key).cloned().unwrap_or(serde_json::Value::Null))
                .map_err(|_| err("invalid", format!("Paramètre invalide : {key}.")))
        }
        fn out<T: Serialize>(v: T) -> Res<serde_json::Value> {
            serde_json::to_value(v).map_err(|_| CommandError::internal())
        }
        match cmd {
            "client_list" => out(self.vault()?.client_list()),
            "client_overview" => out(clients::overview(self.vault()?, arg(args, "on")?)?),
            "client_get" => out(clients::get(self.vault()?, arg(args, "id")?)?),
            "client_save" => out(clients::save(self.vault_mut()?, arg(args, "request")?)?),
            "client_delete" => out(self.vault_mut()?.client_delete(arg(args, "id")?)),
            "client_clear_notes" => {
                let id = arg(args, "id")?;
                self.vault_mut()?.client_set_notes(id, vec![])?;
                out(())
            }
            "assess" => out(clients::assess(
                self.vault()?,
                arg(args, "id")?,
                arg(args, "on")?,
            )?),
            "deid_preview" => out(clients::deid_preview(
                self.vault()?,
                arg(args, "id")?,
                arg(args, "on")?,
            )?),
            "inbox_status" => out(inbox::status(self.vault()?, arg(args, "files")?)),
            "watch_status" => out(watch::status(self.vault()?)?),
            "watch_due" => out(watch::due(self.vault()?)),
            "watch_apply" => out(watch::apply(
                self.vault_mut()?,
                arg(args, "fetch")?,
                arg(args, "published")?,
            )?),
            "watch_failed" => {
                let (fetch, reason) = (arg(args, "fetch")?, arg::<String>(args, "reason")?);
                watch::record_failure(self.vault_mut()?, fetch, &reason);
                out(())
            }
            "watch_approve" => out(watch::approve(
                self.vault_mut()?,
                arg(args, "note")?,
                &time::today(),
            )?),
            "watch_reject" => {
                watch::reject(self.vault_mut()?);
                out(())
            }
            _ => Err(err("unknown_command", "Commande inconnue.")),
        }
    }

    /// Renders the report (refused until every review flag is checked).
    pub fn export_report(&self, request: export::ExportRequest) -> Res<export::ExportFile> {
        export::export(self.vault()?, request)
    }

    /// Imports one form file read from the input folder.
    pub fn inbox_import(&mut self, name: &str, bytes: &[u8]) -> Res<inbox::ImportResult> {
        inbox::import(self.vault_mut()?, name, bytes)
    }
}

pub use export::{ExportFile, ExportRequest};
pub use inbox::fingerprint;

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn lifecycle_and_dirty_tracking() {
        let mut app = App::new(KdfParams::INSECURE_FAST_FOR_TESTS);
        assert_eq!(
            app.call("client_list", &serde_json::Value::Null)
                .unwrap_err()
                .code,
            "locked"
        );
        let created = app.create("a long test passphrase").unwrap();
        assert_eq!(created.recovery_phrase.split(' ').count(), 24);
        assert!(app.take_dirty(), "a new vault must be saved");
        assert!(!app.take_dirty());

        app.call("client_list", &serde_json::Value::Null).unwrap();
        assert!(!app.take_dirty(), "reads do not dirty");

        let profile: serde_json::Value =
            serde_json::from_str(include_str!("../tests/fixtures/ui-full-applicant.json")).unwrap();
        let saved = app
            .call(
                "client_save",
                &serde_json::json!({ "request": {
                    "id": null,
                    "identity": { "full_name": "Hélène Tremblay", "email": null, "phone": null, "file_reference": "2026-041" },
                    "profile": profile,
                }}),
            )
            .unwrap();
        assert_eq!(saved["id"], 1);
        assert!(app.take_dirty());

        let file = app.seal().unwrap();
        assert!(app.lock());
        assert!(!app.is_unlocked());
        app.unlock(&file, "a long test passphrase").unwrap();
        let list = app.call("client_list", &serde_json::Value::Null).unwrap();
        assert_eq!(list[0]["full_name"], "Hélène Tremblay");

        assert_eq!(
            app.call("nope", &serde_json::Value::Null).unwrap_err().code,
            "unknown_command"
        );
        assert_eq!(
            app.call("client_get", &serde_json::json!({ "id": "x" }))
                .unwrap_err()
                .code,
            "invalid"
        );
    }

    #[test]
    fn today_is_a_date() {
        assert!(time::today().as_str() >= "2026-01-01");
    }
}
