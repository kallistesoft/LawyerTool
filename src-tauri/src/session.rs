//! Vault session: unlock/lock state, idle auto-lock, and the vault commands.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use vault::{KdfParams, KeyStore, OsKeyStore, Paths, Vault};
use zeroize::Zeroizing;

/// Auto-lock after this much inactivity.
pub const DEFAULT_IDLE: Duration = Duration::from_secs(5 * 60);
/// Event sent to the window when the vault locks.
pub const LOCKED_EVENT: &str = "vault-locked";

/// Idle tracking, kept separate from Tauri so it can be unit-tested.
#[derive(Debug)]
pub struct Idle {
    last: Instant,
    timeout: Duration,
}

impl Idle {
    pub fn new(timeout: Duration, now: Instant) -> Self {
        Self { last: now, timeout }
    }

    pub fn touch(&mut self, now: Instant) {
        self.last = now;
    }

    pub fn expired(&self, now: Instant) -> bool {
        now.saturating_duration_since(self.last) >= self.timeout
    }
}

pub struct Session {
    paths: Paths,
    vault: Mutex<Option<Vault>>,
    idle: Mutex<Idle>,
    /// Dev build with the automatic test vault (see `dev.rs`): no auto-lock.
    dev: bool,
}

impl Session {
    pub fn new(db: PathBuf, timeout: Duration) -> Self {
        Self {
            paths: Paths::new(db),
            vault: Mutex::new(None),
            idle: Mutex::new(Idle::new(timeout, Instant::now())),
            dev: false,
        }
    }

    /// Dev session: the given (already unlocked) dev vault, never auto-locked.
    #[cfg(debug_assertions)]
    pub fn dev(db: PathBuf, v: Vault) -> Self {
        let s = Self {
            dev: true,
            ..Self::new(db, DEFAULT_IDLE)
        };
        if let Ok(mut slot) = s.vault.lock() {
            *slot = Some(v);
        }
        s
    }

    fn touch(&self) {
        if let Ok(mut i) = self.idle.lock() {
            i.touch(Instant::now());
        }
    }

    /// Drops the open vault (closes the DB, wipes the key). Returns whether it was open.
    pub fn lock(&self) -> bool {
        self.vault
            .lock()
            .map(|mut v| v.take().is_some())
            .unwrap_or(false)
    }

    /// A setting from the vault's `meta` table (`None` if locked or unset).
    pub fn setting(&self, key: &str) -> Option<String> {
        let guard = self.vault.lock().ok()?;
        let v = guard.as_ref()?;
        v.conn()
            .query_row("SELECT value FROM meta WHERE key = ?1", [key], |r| r.get(0))
            .ok()
    }

    fn is_unlocked(&self) -> bool {
        self.vault.lock().map(|v| v.is_some()).unwrap_or(false)
    }

    fn set(&self, v: Vault) -> Result<(), CommandError> {
        *self.vault.lock().map_err(|_| CommandError::internal())? = Some(v);
        self.touch();
        Ok(())
    }
}

/// Audit records go into the unlocked vault. While locked, recording fails,
/// so `net` refuses to report success for any call (no unaudited traffic).
impl audit::AuditSink for Session {
    fn record(&self, r: &audit::AuditRecord) -> Result<(), String> {
        let guard = self
            .vault
            .lock()
            .map_err(|_| "session poisoned".to_owned())?;
        let v = guard.as_ref().ok_or_else(|| "vault is locked".to_owned())?;
        v.audit_record(r).map_err(|e| e.to_string())
    }
}

/// Exact hosts the app may contact (brief: Claude API, official immigration
/// sites, update host). Configurable later; the update host is added in M1 step 7.
pub const ALLOWED_HOSTS: &[&str] = &["api.anthropic.com", "www.canada.ca", "ircc.canada.ca"];

/// Error sent to the frontend: a stable code and a user-facing message.
/// Never contains passphrases, keys or client data.
#[derive(Debug, Serialize)]
pub struct CommandError {
    code: &'static str,
    message: String,
}

impl CommandError {
    fn internal() -> Self {
        Self {
            code: "internal",
            message: "Erreur interne.".into(),
        }
    }
}

impl From<vault::Error> for CommandError {
    fn from(e: vault::Error) -> Self {
        use vault::Error::*;
        let (code, message) = match &e {
            WeakPassphrase => ("weak_passphrase", format!("La phrase de passe doit contenir au moins {} caractères.", vault::MIN_PASSPHRASE_CHARS)),
            WrongPassphrase => ("wrong_passphrase", "Phrase de passe incorrecte.".into()),
            WrongRecoveryPhrase => ("wrong_recovery_phrase", "Phrase de récupération incorrecte.".into()),
            KeychainEntryMissing => ("keychain_missing", "Aucune clé pour ce coffre dans le trousseau de cet ordinateur : utilisez la phrase de récupération.".into()),
            NotFound(_) => ("not_found", "Aucun coffre n'existe encore.".into()),
            AlreadyExists(_) => ("already_exists", "Un coffre existe déjà.".into()),
            NewerSchema { .. } => ("newer_schema", e.to_string()),
            _ => ("vault_error", "Le coffre n'a pas pu être ouvert.".into()),
        };
        Self { code, message }
    }
}

type Res<T> = Result<T, CommandError>;

fn kdf() -> KdfParams {
    KdfParams::DEFAULT
}

/// Runs slow work (Argon2id, SQLCipher) off the main thread.
async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Res<T> + Send + 'static) -> Res<T> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|_| CommandError::internal())?
}

#[derive(Serialize)]
pub struct Status {
    exists: bool,
    unlocked: bool,
    idle_minutes: u64,
    /// Dev build with the automatic test vault.
    dev: bool,
}

#[tauri::command]
pub fn vault_status(s: State<'_, Arc<Session>>) -> Status {
    let idle_minutes = s.idle.lock().map(|i| i.timeout.as_secs() / 60).unwrap_or(0);
    Status {
        exists: s.paths.exists(),
        unlocked: s.is_unlocked(),
        idle_minutes,
        dev: s.dev,
    }
}

#[derive(Serialize)]
pub struct Created {
    recovery_phrase: String,
}

#[tauri::command]
pub async fn vault_create(app: AppHandle, passphrase: String) -> Res<Created> {
    let passphrase = Zeroizing::new(passphrase);
    let paths = app.state::<Arc<Session>>().paths.clone();
    let (v, phrase) = blocking(move || {
        Ok(Vault::create(
            &paths,
            &OsKeyStore as &dyn KeyStore,
            &passphrase,
            kdf(),
        )?)
    })
    .await?;
    app.state::<Arc<Session>>().set(v)?;
    // Shown once by the frontend, never stored.
    Ok(Created {
        recovery_phrase: phrase.as_str().to_owned(),
    })
}

#[tauri::command]
pub async fn vault_unlock(app: AppHandle, passphrase: String) -> Res<()> {
    let passphrase = Zeroizing::new(passphrase);
    let paths = app.state::<Arc<Session>>().paths.clone();
    let v = blocking(move || Ok(Vault::unlock(&paths, &OsKeyStore, &passphrase)?)).await?;
    app.state::<Arc<Session>>().set(v)
}

#[tauri::command]
pub async fn vault_recover(app: AppHandle, phrase: String, new_passphrase: String) -> Res<()> {
    let (phrase, new_passphrase) = (Zeroizing::new(phrase), Zeroizing::new(new_passphrase));
    let paths = app.state::<Arc<Session>>().paths.clone();
    let v = blocking(move || {
        Ok(Vault::recover(
            &paths,
            &OsKeyStore,
            &phrase,
            &new_passphrase,
            kdf(),
        )?)
    })
    .await?;
    app.state::<Arc<Session>>().set(v)
}

#[tauri::command]
pub fn vault_lock(app: AppHandle, s: State<'_, Arc<Session>>) {
    if s.lock() {
        let _ = app.emit(LOCKED_EVENT, ());
    }
}

/// Called by the frontend on user input (throttled), to defer auto-lock.
#[tauri::command]
pub fn activity(s: State<'_, Arc<Session>>) {
    if s.is_unlocked() {
        s.touch();
    }
}

/// How often the idle watch wakes up.
const TICK: Duration = Duration::from_secs(5);
/// A wall-clock jump larger than this between two ticks means the computer
/// slept (or its clock moved): lock, to be safe.
const SLEEP_GAP: Duration = Duration::from_secs(30);

/// `true` if the wall clock advanced much more than one tick: the process was
/// suspended (system sleep). Works the same on macOS and Windows.
pub fn slept(prev: std::time::SystemTime, now: std::time::SystemTime) -> bool {
    match now.duration_since(prev) {
        Ok(d) => d > SLEEP_GAP,
        Err(_) => true, // clock went backwards: treat as suspicious
    }
}

/// Locks the vault after the idle timeout or a system sleep, and tells the window.
pub fn spawn_idle_watch(app: AppHandle) {
    std::thread::spawn(move || {
        let mut last_wall = std::time::SystemTime::now();
        loop {
            std::thread::sleep(TICK);
            let now_wall = std::time::SystemTime::now();
            let woke_from_sleep = slept(last_wall, now_wall);
            last_wall = now_wall;
            let s = app.state::<Arc<Session>>();
            if s.dev {
                continue;
            }
            let expired = woke_from_sleep
                || s.idle
                    .lock()
                    .map(|i| i.expired(Instant::now()))
                    .unwrap_or(true);
            if expired && s.lock() {
                let _ = app.emit(LOCKED_EVENT, ());
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sleep_detection() {
        let t = std::time::SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);
        assert!(!slept(t, t + Duration::from_secs(5)));
        assert!(!slept(t, t + Duration::from_secs(30)));
        assert!(slept(t, t + Duration::from_secs(31)), "laptop lid closed");
        assert!(slept(t, t - Duration::from_secs(1)), "clock moved back");
    }

    #[test]
    fn idle_expiry() {
        let t0 = Instant::now();
        let mut i = Idle::new(Duration::from_secs(300), t0);
        assert!(!i.expired(t0 + Duration::from_secs(299)));
        assert!(i.expired(t0 + Duration::from_secs(300)));
        i.touch(t0 + Duration::from_secs(200));
        assert!(!i.expired(t0 + Duration::from_secs(499)));
        assert!(i.expired(t0 + Duration::from_secs(500)));
    }
}

#[cfg(test)]
mod audit_tests {
    use audit::AuditSink;

    use super::*;

    #[test]
    fn audit_refused_while_locked() {
        let dir = std::env::temp_dir().join(format!("lt-session-{}", std::process::id()));
        let s = Session::new(dir.join("x.db"), DEFAULT_IDLE);
        let r = audit::AuditRecord {
            at: audit::now_rfc3339(),
            method: "GET".into(),
            host: "www.canada.ca".into(),
            category: audit::DataCategory::PublicRulesFetch,
            source_file: None,
            outcome: audit::Outcome::Completed,
            status: Some(200),
            bytes_sent: 0,
            bytes_received: 1,
            reason: None,
        };
        assert_eq!(s.record(&r), Err("vault is locked".to_owned()));
    }
}
