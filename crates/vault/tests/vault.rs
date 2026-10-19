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
    let (v, phrase) = Vault::create(&paths, &store, PASS, KDF).unwrap();
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

#[test]
fn clients_crud_and_encrypted() {
    let (_d, paths, store) = setup();
    let (v, _) = Vault::create(&paths, &store, PASS, KDF).unwrap();
    let who = vault::Identity {
        full_name: "Marie Tremblay-7731".into(),
        email: Some("marie@example.com".into()),
        phone: None,
        file_reference: Some("2026-041".into()),
    };
    let id = v
        .client_save(None, &who, r#"{"age":29}"#, "2026-10-18T10:00:00Z")
        .unwrap();
    let id2 = v
        .client_save(
            None,
            &vault::Identity {
                full_name: "Jean Roy".into(),
                email: None,
                phone: None,
                file_reference: None,
            },
            "{}",
            "2026-10-18T11:00:00Z",
        )
        .unwrap();
    assert_eq!(
        v.client_list()
            .unwrap()
            .iter()
            .map(|c| c.id)
            .collect::<Vec<_>>(),
        vec![id2, id],
        "most recent first"
    );

    let mut who2 = who.clone();
    who2.phone = Some("514-555-0100".into());
    v.client_save(Some(id), &who2, r#"{"age":30}"#, "2026-10-18T12:00:00Z")
        .unwrap();
    let rec = v.client_get(id).unwrap().unwrap();
    assert_eq!(
        (rec.identity, rec.profile.as_str(), rec.created_at.as_str()),
        (who2, r#"{"age":30}"#, "2026-10-18T10:00:00Z")
    );

    assert!(matches!(
        v.client_save(Some(999), &who, "{}", "x"),
        Err(Error::Invalid(_))
    ));
    let blank = vault::Identity {
        full_name: "  ".into(),
        email: None,
        phone: None,
        file_reference: None,
    };
    assert!(matches!(
        v.client_save(None, &blank, "{}", "x"),
        Err(Error::Invalid(_))
    ));

    let long = vault::Identity {
        full_name: "x".repeat(201),
        email: None,
        phone: None,
        file_reference: None,
    };
    assert!(matches!(
        v.client_save(None, &long, "{}", "x"),
        Err(Error::Invalid(_))
    ));
    assert!(matches!(
        v.client_save(None, &who, &" ".repeat(300 * 1024), "x"),
        Err(Error::Invalid(_))
    ));

    assert!(v.client_delete(id2).unwrap());
    assert!(!v.client_delete(id2).unwrap());
    v.lock();
    assert!(
        !contains(&paths.db, b"Tremblay-7731"),
        "client identity is encrypted"
    );
}

#[test]
fn watcher_storage() {
    let (_d, paths, store) = setup();
    let (v, _) = Vault::create(&paths, &store, PASS, KDF).unwrap();
    let draw = |n: &str, date: &str| vault::DrawRow {
        number: n.into(),
        date: date.into(),
        name: "Aucun programme spécifié".into(),
        invitations: 500,
        min_crs: 480,
        first_seen: "2026-10-18".into(),
    };
    assert_eq!(
        v.draws_upsert(&[
            draw("91a", "2018-05-30"),
            draw("91b", "2018-05-30"),
            draw("450", "2026-10-16")
        ])
        .unwrap(),
        3
    );
    assert_eq!(
        v.draws_upsert(&[draw("450", "2026-10-16"), draw("451", "2026-10-20")])
            .unwrap(),
        1,
        "only new draws"
    );
    assert_eq!(
        v.draws_latest(2)
            .unwrap()
            .iter()
            .map(|d| d.number.as_str())
            .collect::<Vec<_>>(),
        ["451", "450"]
    );

    let st = vault::WatchState {
        source_id: "criteries-scg".into(),
        fingerprint: "abc".into(),
        main_text: Some("texte".into()),
        checked_at: "t1".into(),
        changed_at: "t1".into(),
    };
    v.watch_state_set(&st).unwrap();
    v.watch_state_set(&vault::WatchState {
        fingerprint: "def".into(),
        ..st.clone()
    })
    .unwrap();
    assert_eq!(
        v.watch_state_get("criteries-scg")
            .unwrap()
            .unwrap()
            .fingerprint,
        "def"
    );

    v.watch_run_record(&vault::WatchRun {
        at: "t1".into(),
        ok: true,
        summary: "ok".into(),
    })
    .unwrap();
    v.watch_run_record(&vault::WatchRun {
        at: "t2".into(),
        ok: false,
        summary: "offline".into(),
    })
    .unwrap();
    let (last, last_ok) = v.watch_runs_last().unwrap();
    assert_eq!(
        (last.unwrap().at, last_ok.unwrap().at),
        ("t2".to_owned(), "t1".to_owned())
    );

    v.watch_pending_set("t2", "{}").unwrap();
    v.watch_pending_set("t3", r#"{"x":1}"#).unwrap();
    assert_eq!(
        v.watch_pending_get().unwrap(),
        Some(("t3".into(), r#"{"x":1}"#.into()))
    );
    v.watch_pending_clear().unwrap();
    assert_eq!(v.watch_pending_get().unwrap(), None);

    let snap = vault::SnapshotRow {
        id: "crs-x".into(),
        effective_from: "2026-10-18".into(),
        approved_at: "t".into(),
        note: None,
        grid: "{}".into(),
    };
    v.snapshot_save(&snap).unwrap();
    assert!(v.snapshot_save(&snap).is_err(), "snapshot ids are unique");
    assert_eq!(v.snapshot_list().unwrap(), vec![snap]);
}

#[test]
fn imports_are_tracked_by_fingerprint() {
    let (_dir, paths, store) = setup();
    let (v, _) = Vault::create(&paths, &store, PASS, KDF).unwrap();
    assert!(v.import_get("abc").unwrap().is_none());
    let mut row = vault::ImportRow {
        fingerprint: "abc".into(),
        file_name: "reponses.xlsx".into(),
        imported_at: "2026-10-18T10:00:00Z".into(),
        client_id: None,
        outcome: vault::ImportOutcome::Failed,
        message: "format pending".into(),
    };
    v.import_record(&row).unwrap();
    assert_eq!(
        v.import_get("abc").unwrap().unwrap().outcome,
        vault::ImportOutcome::Failed
    );
    let who = vault::Identity {
        full_name: "Test".into(),
        email: None,
        phone: None,
        file_reference: None,
    };
    let id = v.client_save(None, &who, "{}", "x").unwrap();
    row.client_id = Some(id);
    row.outcome = vault::ImportOutcome::Imported;
    v.import_record(&row).unwrap();
    assert_eq!(v.import_get("abc").unwrap().unwrap(), row);
    // Deleting the client keeps the import record (no re-import), unlinked.
    assert!(v.client_delete(id).unwrap());
    assert_eq!(v.import_get("abc").unwrap().unwrap().client_id, None);

    v.meta_set("input_dir", "/x").unwrap();
    v.meta_delete("input_dir").unwrap();
    assert_eq!(v.meta_get("input_dir").unwrap(), None);
}

/// Regression: the migration backup used to overwrite (empty) the live key
/// file, so the vault could never be opened again after an upgrade.
#[test]
fn migration_keeps_the_key_file_and_backs_it_up() {
    let (_dir, paths, store) = setup();
    let (v, _) = Vault::create(&paths, &store, PASS, KDF).unwrap();
    write_marker(&v);
    // Pretend the database predates the last migration.
    let last = vault::schema_version();
    v.conn()
        .execute_batch(&format!(
            "DROP TABLE imports; PRAGMA user_version = {};",
            last - 1
        ))
        .unwrap();
    drop(v);
    let keys_before = std::fs::read(paths.keys()).unwrap();

    let v = Vault::unlock(&paths, &store, PASS).unwrap();
    let backup = v.migration_backup.clone().expect("migration backup");
    assert_eq!(
        std::fs::read(paths.keys()).unwrap(),
        keys_before,
        "live key file untouched"
    );
    assert_eq!(
        std::fs::read(vault::sidecar_keys(&backup)).unwrap(),
        keys_before
    );
    assert_eq!(read_marker(&v), MARKER);
    drop(v);
    // And it still opens afterwards.
    let v = Vault::unlock(&paths, &store, PASS).unwrap();
    assert_eq!(v.schema_version().unwrap(), last);
}

#[test]
fn an_emptied_key_file_is_rebuilt_from_the_keychain() {
    let (_dir, paths, store) = setup();
    let (v, _) = Vault::create(&paths, &store, PASS, KDF).unwrap();
    write_marker(&v);
    drop(v);
    let id: String =
        serde_json::from_slice::<serde_json::Value>(&std::fs::read(paths.keys()).unwrap()).unwrap()
            ["vault_id"]
            .as_str()
            .unwrap()
            .to_owned();
    std::fs::write(paths.keys(), b"").unwrap();
    assert!(matches!(
        Vault::unlock(&paths, &store, PASS),
        Err(Error::Corrupt(_))
    ));
    assert!(matches!(
        Vault::rebuild_keyfile(&paths, &store, &id, "wrong passphrase!"),
        Err(Error::WrongPassphrase)
    ));

    let (v, phrase) = Vault::rebuild_keyfile(&paths, &store, &id, PASS).unwrap();
    assert_eq!(read_marker(&v), MARKER);
    drop(v);
    // Normal unlock works again, and the new recovery phrase is valid.
    drop(Vault::unlock(&paths, &store, PASS).unwrap());
    let store2 = MemoryKeyStore::default();
    let v = Vault::recover(&paths, &store2, phrase.as_str(), "another passphrase", KDF).unwrap();
    assert_eq!(read_marker(&v), MARKER);
}
