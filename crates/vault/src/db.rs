//! SQLCipher connection, schema migrations, encrypted backup.

use std::path::{Path, PathBuf};

use rusqlite::Connection;

use crate::Error;
use crate::crypto::Secret;

/// Ordered schema migrations; migration `i` brings the schema to version `i + 1`.
pub const MIGRATIONS: &[&str] = &[
    // v1: key/value metadata.
    "CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL) STRICT;",
    // v2: outbound-call audit log (metadata only, never content).
    "CREATE TABLE audit_log (
        id INTEGER PRIMARY KEY,
        at TEXT NOT NULL,
        method TEXT NOT NULL,
        host TEXT NOT NULL,
        category TEXT NOT NULL,
        source_file TEXT,
        outcome TEXT NOT NULL,
        status INTEGER,
        bytes_sent INTEGER NOT NULL,
        bytes_received INTEGER NOT NULL,
        reason TEXT
    ) STRICT;
    CREATE INDEX audit_log_at ON audit_log (at);",
    // v3: clients. Identity fields are kept apart from the profile (JSON of
    // applicant::Applicant), which never holds names or contact details, so
    // it can be de-identified for Claude.
    "CREATE TABLE clients (
        id INTEGER PRIMARY KEY,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL,
        full_name TEXT NOT NULL,
        email TEXT,
        phone TEXT,
        file_reference TEXT,
        profile TEXT NOT NULL
    ) STRICT;",
    // v4: rules watcher (public data only): approved grid snapshots, draws,
    // per-source state, pending change set, run history.
    "CREATE TABLE grid_snapshots (
        id TEXT PRIMARY KEY,
        effective_from TEXT NOT NULL,
        approved_at TEXT NOT NULL,
        note TEXT,
        grid TEXT NOT NULL
    ) STRICT;
    CREATE TABLE draws (
        number TEXT PRIMARY KEY,
        date TEXT NOT NULL,
        name TEXT NOT NULL,
        invitations INTEGER NOT NULL,
        min_crs INTEGER NOT NULL,
        first_seen TEXT NOT NULL
    ) STRICT;
    CREATE INDEX draws_date ON draws (date);
    CREATE TABLE watch_state (
        source_id TEXT PRIMARY KEY,
        fingerprint TEXT NOT NULL,
        main_text TEXT,
        checked_at TEXT NOT NULL,
        changed_at TEXT NOT NULL
    ) STRICT;
    CREATE TABLE watch_runs (
        id INTEGER PRIMARY KEY,
        at TEXT NOT NULL,
        ok INTEGER NOT NULL,
        summary TEXT NOT NULL
    ) STRICT;
    CREATE TABLE watch_pending (
        id INTEGER PRIMARY KEY CHECK (id = 1),
        created_at TEXT NOT NULL,
        payload TEXT NOT NULL
    ) STRICT;",
    // v5: form files seen in the input folder (by content fingerprint; the
    // file content itself is never stored here).
    "CREATE TABLE imports (
        fingerprint TEXT PRIMARY KEY,
        file_name TEXT NOT NULL,
        imported_at TEXT NOT NULL,
        client_id INTEGER REFERENCES clients (id) ON DELETE SET NULL,
        outcome TEXT NOT NULL,
        message TEXT NOT NULL
    ) STRICT;",
];

pub fn schema_version() -> u32 {
    u32::try_from(MIGRATIONS.len()).unwrap_or(u32::MAX)
}

/// Opens `path` with `dek`. A wrong key surfaces as `Error::WrongKey`.
pub fn open(path: &Path, dek: &Secret) -> Result<Connection, Error> {
    let conn = Connection::open(path)?;
    let key = dek.sqlcipher_literal();
    conn.execute_batch(&format!("PRAGMA key = \"{}\";", key.as_str()))?;
    // Fails with "file is not a database" if the key is wrong.
    if conn
        .query_row("SELECT count(*) FROM sqlite_master", [], |r| {
            r.get::<_, i64>(0)
        })
        .is_err()
    {
        return Err(Error::WrongKey);
    }
    // cipher_memory_security stays OFF (SQLCipher's default). With it ON, on
    // Windows VirtualLock fails (quota, error 1453) and SQLCipher's warning
    // path allocates again, recursing until the stack overflows (CI,
    // 2026-10-18). Keys are wiped by Zeroize; swap is covered by the required
    // full-disk encryption.
    // temp_store = MEMORY: SQLite's temporary sort/index files never reach the
    // disk (brief rule 9: no sensitive data in temp files).
    conn.execute_batch(
        "PRAGMA foreign_keys = ON; PRAGMA secure_delete = ON; PRAGMA temp_store = MEMORY;",
    )?;
    Ok(conn)
}

fn user_version(conn: &Connection) -> Result<u32, Error> {
    Ok(conn.query_row("PRAGMA user_version", [], |r| r.get(0))?)
}

fn sql_string(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

/// Encrypted copy of the open database (same key) at `dest`.
pub fn export(conn: &Connection, dest: &Path, dek: &Secret) -> Result<(), Error> {
    if dest.exists() {
        return Err(Error::AlreadyExists(dest.to_path_buf()));
    }
    let path = dest
        .to_str()
        .ok_or(Error::Corrupt("non-UTF-8 backup path"))?;
    let key = dek.sqlcipher_literal();
    // sqlcipher_export copies schema and data but not user_version; copy it
    // explicitly, or a restored backup would look like an empty v0 database.
    let version = user_version(conn)?;
    conn.execute_batch(&format!(
        "ATTACH DATABASE {} AS backup KEY \"{}\"; SELECT sqlcipher_export('backup'); PRAGMA backup.user_version = {version}; DETACH DATABASE backup;",
        sql_string(path),
        key.as_str()
    ))?;
    Ok(())
}

/// Next free `<db>.backup-v<from>[-n]` path.
fn backup_path(db: &Path, from: u32) -> PathBuf {
    let base = db.with_extension(format!("backup-v{from}"));
    if !base.exists() {
        return base;
    }
    (1..)
        .map(|n| db.with_extension(format!("backup-v{from}-{n}")))
        .find(|p| !p.exists())
        .unwrap_or(base)
}

/// Applies pending migrations, after an encrypted backup when the database
/// already holds data. Returns the backup path, if one was made.
pub fn migrate(conn: &mut Connection, db: &Path, dek: &Secret) -> Result<Option<PathBuf>, Error> {
    migrate_with(conn, db, dek, MIGRATIONS)
}

pub(crate) fn migrate_with(
    conn: &mut Connection,
    db: &Path,
    dek: &Secret,
    migrations: &[&str],
) -> Result<Option<PathBuf>, Error> {
    let current = user_version(conn)?;
    let target = u32::try_from(migrations.len()).unwrap_or(u32::MAX);
    if current > target {
        return Err(Error::NewerSchema {
            found: current,
            supported: target,
        });
    }
    if current == target {
        return Ok(None);
    }
    let backup = if current > 0 {
        let p = backup_path(db, current);
        export(conn, &p, dek)?;
        Some(p)
    } else {
        None
    };
    for (i, sql) in migrations.iter().enumerate().skip(current as usize) {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.execute_batch(&format!("PRAGMA user_version = {}", i + 1))?;
        tx.commit()?;
    }
    Ok(backup)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn migration_takes_encrypted_backup_first() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.db");
        let dek = Secret::random().unwrap();
        let v1: &[&str] = &["CREATE TABLE notes (body TEXT NOT NULL) STRICT;"];
        let v2: &[&str] = &[v1[0], "ALTER TABLE notes ADD COLUMN author TEXT;"];

        let mut conn = open(&path, &dek).unwrap();
        assert_eq!(
            migrate_with(&mut conn, &path, &dek, v1).unwrap(),
            None,
            "fresh DB: no backup"
        );
        conn.execute("INSERT INTO notes (body) VALUES ('before upgrade')", [])
            .unwrap();

        let backup = migrate_with(&mut conn, &path, &dek, v2)
            .unwrap()
            .expect("backup before migrating data");
        assert_eq!(user_version(&conn).unwrap(), 2);

        // The backup is encrypted with the same key, still at v1, with the data.
        let b = open(&backup, &dek).unwrap();
        assert_eq!(user_version(&b).unwrap(), 1);
        let body: String = b
            .query_row("SELECT body FROM notes", [], |r| r.get(0))
            .unwrap();
        assert_eq!(body, "before upgrade");
        assert!(matches!(
            open(&backup, &Secret::random().unwrap()),
            Err(Error::WrongKey)
        ));
        assert!(
            !std::fs::read(&backup)
                .unwrap()
                .windows(14)
                .any(|w| w == b"before upgrade")
        );

        // A second migration does not overwrite the first backup.
        let v3: &[&str] = &[v2[0], v2[1], "CREATE INDEX notes_body ON notes (body);"];
        let second = migrate_with(&mut conn, &path, &dek, v3).unwrap().unwrap();
        assert_ne!(second, backup);
    }

    #[test]
    fn refuses_newer_schema() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.db");
        let dek = Secret::random().unwrap();
        let mut conn = open(&path, &dek).unwrap();
        conn.execute_batch("PRAGMA user_version = 99").unwrap();
        assert!(matches!(
            migrate(&mut conn, &path, &dek),
            Err(Error::NewerSchema { found: 99, .. })
        ));
    }
}

#[cfg(test)]
mod memory_security {
    #![allow(clippy::unwrap_used)]

    /// Regression guard: memory security must stay off (see `open`).
    #[test]
    fn cipher_memory_security_is_off() {
        let dir = tempfile::tempdir().unwrap();
        let conn =
            super::open(&dir.path().join("t.db"), &super::Secret::random().unwrap()).unwrap();
        let v: String = conn
            .query_row("PRAGMA cipher_memory_security", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, "0");
        let t: i64 = conn
            .query_row("PRAGMA temp_store", [], |r| r.get(0))
            .unwrap();
        assert_eq!(t, 2, "temp_store = MEMORY");
    }
}
