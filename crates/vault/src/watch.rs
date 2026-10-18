//! Storage for the rules watcher. Public data only (official pages, draws,
//! approved rules); kept in the vault so everything lives in one encrypted
//! file. JSON payloads are opaque here.

use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

use crate::{Error, Vault};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotRow {
    pub id: String,
    pub effective_from: String,
    pub approved_at: String,
    pub note: Option<String>,
    /// JSON of rules_engine::grid::Grid.
    pub grid: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DrawRow {
    pub number: String,
    pub date: String,
    pub name: String,
    pub invitations: u32,
    pub min_crs: u16,
    pub first_seen: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WatchState {
    pub source_id: String,
    pub fingerprint: String,
    pub main_text: Option<String>,
    pub checked_at: String,
    pub changed_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WatchRun {
    pub at: String,
    pub ok: bool,
    pub summary: String,
}

impl Vault {
    /// A value from the key/value `meta` table.
    pub fn meta_get(&self, key: &str) -> Result<Option<String>, Error> {
        Ok(self
            .conn()
            .query_row("SELECT value FROM meta WHERE key = ?1", [key], |r| r.get(0))
            .optional()?)
    }

    pub fn meta_set(&self, key: &str, value: &str) -> Result<(), Error> {
        self.conn().execute(
            "INSERT INTO meta (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = ?2",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn snapshot_save(&self, s: &SnapshotRow) -> Result<(), Error> {
        self.conn().execute(
            "INSERT INTO grid_snapshots (id, effective_from, approved_at, note, grid) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![s.id, s.effective_from, s.approved_at, s.note, s.grid],
        )?;
        Ok(())
    }

    pub fn snapshot_list(&self) -> Result<Vec<SnapshotRow>, Error> {
        let mut st = self
            .conn()
            .prepare("SELECT id, effective_from, approved_at, note, grid FROM grid_snapshots ORDER BY effective_from, approved_at")?;
        let rows = st.query_map([], |r| {
            Ok(SnapshotRow {
                id: r.get(0)?,
                effective_from: r.get(1)?,
                approved_at: r.get(2)?,
                note: r.get(3)?,
                grid: r.get(4)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Inserts draws not seen before. Returns how many were new.
    pub fn draws_upsert(&self, draws: &[DrawRow]) -> Result<usize, Error> {
        let mut n = 0;
        for d in draws {
            n += self.conn().execute(
                "INSERT OR IGNORE INTO draws (number, date, name, invitations, min_crs, first_seen) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![d.number, d.date, d.name, d.invitations, d.min_crs, d.first_seen],
            )?;
        }
        Ok(n)
    }

    /// Most recent draws first.
    pub fn draws_latest(&self, limit: u32) -> Result<Vec<DrawRow>, Error> {
        let mut st = self.conn().prepare(
            "SELECT number, date, name, invitations, min_crs, first_seen FROM draws
             ORDER BY date DESC, CAST(number AS INTEGER) DESC, number DESC LIMIT ?1",
        )?;
        let rows = st.query_map([limit], |r| {
            Ok(DrawRow {
                number: r.get(0)?,
                date: r.get(1)?,
                name: r.get(2)?,
                invitations: r.get(3)?,
                min_crs: r.get(4)?,
                first_seen: r.get(5)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn watch_state_get(&self, source_id: &str) -> Result<Option<WatchState>, Error> {
        Ok(self
            .conn()
            .query_row(
                "SELECT source_id, fingerprint, main_text, checked_at, changed_at FROM watch_state WHERE source_id = ?1",
                [source_id],
                |r| {
                    Ok(WatchState {
                        source_id: r.get(0)?,
                        fingerprint: r.get(1)?,
                        main_text: r.get(2)?,
                        checked_at: r.get(3)?,
                        changed_at: r.get(4)?,
                    })
                },
            )
            .optional()?)
    }

    pub fn watch_state_set(&self, s: &WatchState) -> Result<(), Error> {
        self.conn().execute(
            "INSERT INTO watch_state (source_id, fingerprint, main_text, checked_at, changed_at) VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(source_id) DO UPDATE SET fingerprint = ?2, main_text = ?3, checked_at = ?4, changed_at = ?5",
            params![s.source_id, s.fingerprint, s.main_text, s.checked_at, s.changed_at],
        )?;
        Ok(())
    }

    pub fn watch_run_record(&self, r: &WatchRun) -> Result<(), Error> {
        self.conn().execute(
            "INSERT INTO watch_runs (at, ok, summary) VALUES (?1, ?2, ?3)",
            params![r.at, r.ok, r.summary],
        )?;
        Ok(())
    }

    /// The last run, and the last successful one.
    pub fn watch_runs_last(&self) -> Result<(Option<WatchRun>, Option<WatchRun>), Error> {
        let q = |sql: &str| {
            self.conn()
                .query_row(sql, [], |r| {
                    Ok(WatchRun {
                        at: r.get(0)?,
                        ok: r.get(1)?,
                        summary: r.get(2)?,
                    })
                })
                .optional()
        };
        Ok((
            q("SELECT at, ok, summary FROM watch_runs ORDER BY id DESC LIMIT 1")?,
            q("SELECT at, ok, summary FROM watch_runs WHERE ok = 1 ORDER BY id DESC LIMIT 1")?,
        ))
    }

    pub fn watch_pending_set(&self, created_at: &str, payload: &str) -> Result<(), Error> {
        self.conn().execute(
            "INSERT INTO watch_pending (id, created_at, payload) VALUES (1, ?1, ?2)
             ON CONFLICT(id) DO UPDATE SET created_at = ?1, payload = ?2",
            params![created_at, payload],
        )?;
        Ok(())
    }

    pub fn watch_pending_get(&self) -> Result<Option<(String, String)>, Error> {
        Ok(self
            .conn()
            .query_row(
                "SELECT created_at, payload FROM watch_pending WHERE id = 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?)
    }

    pub fn watch_pending_clear(&self) -> Result<(), Error> {
        self.conn().execute("DELETE FROM watch_pending", [])?;
        Ok(())
    }
}
