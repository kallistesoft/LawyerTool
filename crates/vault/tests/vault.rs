#![allow(clippy::unwrap_used, clippy::expect_used)] // test code may panic

use vault::{Error, KdfParams, KeyStore, MemoryKeyStore, OsKeyStore, Paths, Vault};

const KDF: KdfParams = KdfParams::INSECURE_FAST_FOR_TESTS;
const PASS: &str = "correct horse battery staple";
const MARKER: &str = "CONFIDENTIEL-Dossier-Tremblay-7731";

fn setup() -> (tempfile::TempDir, Paths, MemoryKeyStore) {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path().join("lawyertool.db"));
    (dir, paths, MemoryKeyStore::default())
}

fn write_marker(v: &Vault) {
    v.conn()
        .execute(
            "INSERT INTO meta (key, value) VALUES ('client', ?1)",
            [MARKER],
        )
        .unwrap();
}

fn read_marker(v: &Vault) -> String {
    v.conn()
        .query_row("SELECT value FROM meta WHERE key = 'client'", [], |r| {
            r.get(0)
        })
        .unwrap()
}

fn contains(path: &std::path::Path, needle: &[u8]) -> bool {
    std::fs::read(path)
        .unwrap()
        .windows(needle.len())
        .any(|w| w == needle)
}

#[test]
fn create_unlock_and_wrong_passphrase() {
    let (_d, paths, store) = setup();
    let (v, phrase) = Vault::create(&paths, &store, PASS, KDF).unwrap();
    assert_eq!(phrase.as_str().split(' ').count(), 24);
    assert_eq!(v.schema_version().unwrap(), vault::schema_version());
    write_marker(&v);
    v.lock();

    let v = Vault::unlock(&paths, &store, PASS).unwrap();
    assert_eq!(read_marker(&v), MARKER);
    drop(v);
    assert!(matches!(
        Vault::unlock(&paths, &store, "correct horse battery stapl"),
        Err(Error::WrongPassphrase)
    ));
}

#[test]
fn weak_passphrase_and_existing_vault_are_refused() {
    let (_d, paths, store) = setup();
    assert!(matches!(
        Vault::create(&paths, &store, "short", KDF),
        Err(Error::WeakPassphrase)
    ));
    let _ = Vault::create(&paths, &store, PASS, KDF).unwrap();
    assert!(matches!(
        Vault::create(&paths, &store, PASS, KDF),
        Err(Error::AlreadyExists(_))
    ));
}

#[test]
fn nothing_readable_at_rest() {
    let (_d, paths, store) = setup();
    let (v, phrase) = Vault::create(&paths, &store, PASS, KDF).unwrap();
    write_marker(&v);
    v.lock();

    let db = std::fs::read(&paths.db).unwrap();
    assert!(
        !db.starts_with(b"SQLite format 3"),
        "no plaintext SQLite header"
    );
    assert!(
        !contains(&paths.db, MARKER.as_bytes()),
        "client data is encrypted"
    );
    for f in [&paths.db, &paths.keys()] {
        assert!(
            !contains(f, PASS.as_bytes()),
            "{f:?} must not contain the passphrase"
        );
        let first_word = phrase.as_str().split(' ').next().unwrap();
        assert!(
            !contains(f, phrase.as_str().as_bytes())
                && !contains(f, format!("\"{first_word} ").as_bytes())
        );
    }
    // The keychain entry holds only the wrapped key, never the passphrase.
    let entry = String::from_utf8(store.get(&keychain_account(&paths)).unwrap().unwrap()).unwrap();
    assert!(!entry.contains(PASS));
}

fn keychain_account(paths: &Paths) -> String {
    let keys: serde_json::Value =
        serde_json::from_slice(&std::fs::read(paths.keys()).unwrap()).unwrap();
    keys["vault_id"].as_str().unwrap().to_owned()
}

#[test]
fn recovery_after_keychain_loss() {
    let (_d, paths, store) = setup();
    let (v, phrase) = Vault::create(&paths, &store, PASS, KDF).unwrap();
    write_marker(&v);
    v.lock();

    store.delete(&keychain_account(&paths)).unwrap();
    assert!(matches!(
        Vault::unlock(&paths, &store, PASS),
        Err(Error::KeychainEntryMissing)
    ));

    // Wrong or mistyped phrases are refused.
    assert!(matches!(
        Vault::recover(
            &paths,
            &store,
            "abandon ability able",
            "a new long passphrase",
            KDF
        ),
        Err(Error::WrongRecoveryPhrase)
    ));
    let mut words: Vec<&str> = phrase.as_str().split(' ').collect();
    words.swap(0, 1);
    let swapped = words.join(" ");
    assert!(matches!(
        Vault::recover(&paths, &store, &swapped, "a new long passphrase", KDF),
        Err(Error::WrongRecoveryPhrase)
    ));

    // Extra spaces and capitals are tolerated.
    let messy = format!("  {}  ", phrase.as_str().to_uppercase().replace(' ', "   "));
    let v = Vault::recover(&paths, &store, &messy, "a new long passphrase", KDF).unwrap();
    assert_eq!(read_marker(&v), MARKER);
    v.lock();
    assert!(Vault::unlock(&paths, &store, "a new long passphrase").is_ok());
    assert!(matches!(
        Vault::unlock(&paths, &store, PASS),
        Err(Error::WrongPassphrase)
    ));
}

#[test]
fn backup_restores_on_a_new_computer() {
    let (dir, paths, store) = setup();
    let (mut v, phrase) = Vault::create(&paths, &store, PASS, KDF).unwrap();
    write_marker(&v);
    let dest = dir.path().join("backup.db");
    v.backup(&dest).unwrap();
    assert!(
        matches!(v.backup(&dest), Err(Error::AlreadyExists(_))),
        "never overwrites"
    );
    v.lock();
    assert!(!contains(&dest, MARKER.as_bytes()), "backup is encrypted");

    // Another computer: empty keychain. The passphrase alone is not enough.
    let other = MemoryKeyStore::default();
    let restored = Paths::new(dest);
    assert!(matches!(
        Vault::unlock(&restored, &other, PASS),
        Err(Error::KeychainEntryMissing)
    ));
    let v = Vault::recover(&restored, &other, phrase.as_str(), PASS, KDF).unwrap();
    assert_eq!(read_marker(&v), MARKER);
}

#[test]
fn change_passphrase() {
    let (_d, paths, store) = setup();
    let (v, _) = Vault::create(&paths, &store, PASS, KDF).unwrap();
    assert!(matches!(
        v.change_passphrase(&store, "too short", KDF),
        Err(Error::WeakPassphrase)
    ));
    v.change_passphrase(&store, "another long passphrase", KDF)
        .unwrap();
    v.lock();
    assert!(Vault::unlock(&paths, &store, "another long passphrase").is_ok());
    assert!(matches!(
        Vault::unlock(&paths, &store, PASS),
        Err(Error::WrongPassphrase)
    ));
}

#[test]
fn tampered_key_file_is_refused() {
    let (_d, paths, store) = setup();
    let (_, phrase) = Vault::create(&paths, &store, PASS, KDF).unwrap();
    let mut keys: serde_json::Value =
        serde_json::from_slice(&std::fs::read(paths.keys()).unwrap()).unwrap();
    keys["vault_id"] = "00000000000000000000000000000000".into();
    std::fs::write(paths.keys(), serde_json::to_vec(&keys).unwrap()).unwrap();
    // The wrapped key is bound to its vault id: moving or editing it fails.
    assert!(matches!(
        Vault::recover(&paths, &store, phrase.as_str(), PASS, KDF),
        Err(Error::WrongRecoveryPhrase)
    ));
    std::fs::write(paths.keys(), b"{not json").unwrap();
    assert!(matches!(
        Vault::unlock(&paths, &store, PASS),
        Err(Error::Corrupt(_))
    ));
}

#[test]
fn missing_vault() {
    let (_d, paths, store) = setup();
    assert!(matches!(
        Vault::unlock(&paths, &store, PASS),
        Err(Error::NotFound(_))
    ));
}

/// Touches the real OS keychain: run manually with `--include-ignored`.
#[test]
#[ignore = "uses the real OS keychain"]
fn os_keychain_roundtrip() {
    let account = format!("test-{}", std::process::id());
    let store = OsKeyStore;
    store.set(&account, b"wrapped-key-bytes").unwrap();
    assert_eq!(
        store.get(&account).unwrap().as_deref(),
        Some(&b"wrapped-key-bytes"[..])
    );
    store.delete(&account).unwrap();
    assert_eq!(store.get(&account).unwrap(), None);
}

#[test]
fn audit_log_roundtrip_is_encrypted() {
    let (_d, paths, store) = setup();
    let (v, _) = Vault::create(&paths, &store, PASS, KDF).unwrap();
    let rec = audit::AuditRecord {
        at: "2026-10-18T17:39:05Z".into(),
        method: "POST".into(),
        host: "api.anthropic.com".into(),
        category: audit::DataCategory::DeidentifiedClientData,
        source_file: Some("Formulaire-Tremblay-7731.xlsx".into()),
        outcome: audit::Outcome::Completed,
        status: Some(200),
        bytes_sent: 1234,
        bytes_received: 5678,
        reason: None,
    };
    v.audit_record(&rec).unwrap();
    assert_eq!(v.audit_list(10).unwrap(), vec![rec]);
    v.lock();
    assert!(
        !contains(&paths.db, b"Tremblay-7731"),
        "audit log is inside the encrypted DB"
    );
}
