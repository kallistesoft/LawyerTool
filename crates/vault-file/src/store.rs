//! The decrypted vault content, held in memory while unlocked. Same records
//! and rules as the desktop vault's tables (`crates/vault`), so the commands
//! behave the same.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::Error;

/// Version of the body layout below.
pub const SCHEMA: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Identity {
    pub full_name: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    /// The firm's file number, if any.
    pub file_reference: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientRecord {
    pub id: i64,
    pub created_at: String,
    pub updated_at: String,
    pub identity: Identity,
    /// JSON of the applicant profile.
    pub profile: String,
    /// What the lawyer must complete or check (set by a form import, kept
    /// until the lawyer clears it).
    #[serde(default)]
    pub notes: Vec<String>,
    /// Preliminary-score inputs from a form import (JSON: completed profile,
    /// assumed language levels, assumptions), if any.
    #[serde(default)]
    pub estimate: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientSummary {
    pub id: i64,
    pub full_name: String,
    pub file_reference: Option<String>,
    pub updated_at: String,
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportOutcome {
    /// A client was created from the file.
    Imported,
    /// The file could not be imported (format, validation…); retried on
    /// the next "Analyser".
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportRow {
    pub fingerprint: String,
    pub file_name: String,
    pub imported_at: String,
    pub client_id: Option<i64>,
    pub outcome: ImportOutcome,
    pub message: String,
}

/// Input limits (characters / bytes): generous for real data, small enough
/// that a hostile or corrupted import cannot bloat the vault.
pub const MAX_NAME: usize = 200;
pub const MAX_EMAIL: usize = 254;
pub const MAX_PHONE: usize = 40;
pub const MAX_FILE_REFERENCE: usize = 64;
pub const MAX_PROFILE_BYTES: usize = 256 * 1024;
/// Watch runs kept (oldest dropped first).
const MAX_RUNS: usize = 200;
/// Audit records kept (oldest dropped first).
const MAX_AUDIT: usize = 5_000;

fn too_long(v: &Option<String>, max: usize) -> bool {
    v.as_ref().is_some_and(|s| s.chars().count() > max)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Store {
    pub schema: u32,
    next_client_id: i64,
    clients: Vec<ClientRecord>,
    meta: BTreeMap<String, String>,
    audit_log: Vec<audit::AuditRecord>,
    grid_snapshots: Vec<SnapshotRow>,
    draws: Vec<DrawRow>,
    watch_state: BTreeMap<String, WatchState>,
    watch_runs: Vec<WatchRun>,
    /// (created_at, payload JSON).
    watch_pending: Option<(String, String)>,
    imports: BTreeMap<String, ImportRow>,
}

impl Default for Store {
    fn default() -> Self {
        Self {
            schema: SCHEMA,
            next_client_id: 1,
            clients: vec![],
            meta: BTreeMap::new(),
            audit_log: vec![],
            grid_snapshots: vec![],
            draws: vec![],
            watch_state: BTreeMap::new(),
            watch_runs: vec![],
            watch_pending: None,
            imports: BTreeMap::new(),
        }
    }
}

impl Store {
    /// Inserts (`id = None`) or updates a client. Returns its id.
    pub fn client_save(
        &mut self,
        id: Option<i64>,
        identity: &Identity,
        profile: &str,
        now: &str,
    ) -> Result<i64, Error> {
        if identity.full_name.trim().is_empty() {
            return Err(Error::Invalid("le nom du client est obligatoire"));
        }
        if identity.full_name.chars().count() > MAX_NAME
            || too_long(&identity.email, MAX_EMAIL)
            || too_long(&identity.phone, MAX_PHONE)
            || too_long(&identity.file_reference, MAX_FILE_REFERENCE)
        {
            return Err(Error::Invalid(
                "un champ d’identité est trop long (nom 200, courriel 254, téléphone 40, dossier 64 caractères au plus)",
            ));
        }
        if profile.len() > MAX_PROFILE_BYTES {
            return Err(Error::Invalid("profil trop volumineux"));
        }
        let identity = Identity {
            full_name: identity.full_name.trim().to_owned(),
            ..identity.clone()
        };
        match id {
            None => {
                let id = self.next_client_id;
                self.next_client_id += 1;
                self.clients.push(ClientRecord {
                    id,
                    created_at: now.to_owned(),
                    updated_at: now.to_owned(),
                    identity,
                    profile: profile.to_owned(),
                    notes: vec![],
                    estimate: None,
                });
                Ok(id)
            }
            Some(id) => {
                let c = self
                    .clients
                    .iter_mut()
                    .find(|c| c.id == id)
                    .ok_or(Error::Invalid("client introuvable"))?;
                c.updated_at = now.to_owned();
                c.identity = identity;
                c.profile = profile.to_owned();
                Ok(id)
            }
        }
    }

    /// Sets (or clears, with an empty list) a client's to-complete notes.
    pub fn client_set_notes(&mut self, id: i64, notes: Vec<String>) -> Result<(), Error> {
        let c = self
            .clients
            .iter_mut()
            .find(|c| c.id == id)
            .ok_or(Error::Invalid("client introuvable"))?;
        c.notes = notes;
        Ok(())
    }

    /// Sets a client's preliminary-score inputs (JSON), or removes them.
    pub fn client_set_estimate(&mut self, id: i64, estimate: Option<String>) -> Result<(), Error> {
        let c = self
            .clients
            .iter_mut()
            .find(|c| c.id == id)
            .ok_or(Error::Invalid("client introuvable"))?;
        c.estimate = estimate;
        Ok(())
    }

    pub fn client_get(&self, id: i64) -> Option<ClientRecord> {
        self.clients.iter().find(|c| c.id == id).cloned()
    }

    /// Most recently updated first.
    pub fn client_list(&self) -> Vec<ClientSummary> {
        let mut rows: Vec<ClientSummary> = self
            .clients
            .iter()
            .map(|c| ClientSummary {
                id: c.id,
                full_name: c.identity.full_name.clone(),
                file_reference: c.identity.file_reference.clone(),
                updated_at: c.updated_at.clone(),
            })
            .collect();
        rows.sort_by(|a, b| b.updated_at.cmp(&a.updated_at).then(b.id.cmp(&a.id)));
        rows
    }

    pub fn client_delete(&mut self, id: i64) -> bool {
        let before = self.clients.len();
        self.clients.retain(|c| c.id != id);
        self.clients.len() < before
    }

    /// A value from the key/value settings.
    pub fn meta_get(&self, key: &str) -> Option<String> {
        self.meta.get(key).cloned()
    }

    pub fn meta_set(&mut self, key: &str, value: &str) {
        self.meta.insert(key.to_owned(), value.to_owned());
    }

    pub fn meta_delete(&mut self, key: &str) {
        self.meta.remove(key);
    }

    /// Appends an audit record (see the `audit` crate: metadata only).
    pub fn audit_record(&mut self, r: &audit::AuditRecord) {
        self.audit_log.push(r.clone());
        let excess = self.audit_log.len().saturating_sub(MAX_AUDIT);
        self.audit_log.drain(..excess);
    }

    /// Most recent audit records first.
    pub fn audit_list(&self, limit: usize) -> Vec<audit::AuditRecord> {
        self.audit_log.iter().rev().take(limit).cloned().collect()
    }

    pub fn snapshot_save(&mut self, s: &SnapshotRow) -> Result<(), Error> {
        if self.grid_snapshots.iter().any(|x| x.id == s.id) {
            return Err(Error::Invalid("grille déjà approuvée"));
        }
        self.grid_snapshots.push(s.clone());
        Ok(())
    }

    /// Ordered by effective date, then approval time.
    pub fn snapshot_list(&self) -> Vec<SnapshotRow> {
        let mut rows = self.grid_snapshots.clone();
        rows.sort_by(|a, b| {
            (a.effective_from.as_str(), a.approved_at.as_str())
                .cmp(&(b.effective_from.as_str(), b.approved_at.as_str()))
        });
        rows
    }

    /// Inserts draws not seen before (by number). Returns how many were new.
    pub fn draws_upsert(&mut self, draws: &[DrawRow]) -> usize {
        let mut n = 0;
        for d in draws {
            if !self.draws.iter().any(|x| x.number == d.number) {
                self.draws.push(d.clone());
                n += 1;
            }
        }
        n
    }

    /// Most recent draws first.
    pub fn draws_latest(&self, limit: usize) -> Vec<DrawRow> {
        let mut rows = self.draws.clone();
        let num = |d: &DrawRow| d.number.parse::<i64>().unwrap_or(0);
        rows.sort_by(|a, b| {
            b.date
                .cmp(&a.date)
                .then(num(b).cmp(&num(a)))
                .then(b.number.cmp(&a.number))
        });
        rows.truncate(limit);
        rows
    }

    pub fn watch_state_get(&self, source_id: &str) -> Option<WatchState> {
        self.watch_state.get(source_id).cloned()
    }

    pub fn watch_state_set(&mut self, s: &WatchState) {
        self.watch_state.insert(s.source_id.clone(), s.clone());
    }

    pub fn watch_run_record(&mut self, r: &WatchRun) {
        self.watch_runs.push(r.clone());
        let excess = self.watch_runs.len().saturating_sub(MAX_RUNS);
        self.watch_runs.drain(..excess);
    }

    /// The last run, and the last successful one.
    pub fn watch_runs_last(&self) -> (Option<WatchRun>, Option<WatchRun>) {
        (
            self.watch_runs.last().cloned(),
            self.watch_runs.iter().rev().find(|r| r.ok).cloned(),
        )
    }

    pub fn watch_pending_set(&mut self, created_at: &str, payload: &str) {
        self.watch_pending = Some((created_at.to_owned(), payload.to_owned()));
    }

    pub fn watch_pending_get(&self) -> Option<(String, String)> {
        self.watch_pending.clone()
    }

    pub fn watch_pending_clear(&mut self) {
        self.watch_pending = None;
    }

    /// Records (or replaces) the latest attempt for a fingerprint.
    pub fn import_record(&mut self, row: &ImportRow) {
        self.imports.insert(row.fingerprint.clone(), row.clone());
    }

    /// The latest attempt for a fingerprint, if any.
    pub fn import_get(&self, fingerprint: &str) -> Option<ImportRow> {
        self.imports.get(fingerprint).cloned()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    fn id(name: &str) -> Identity {
        Identity {
            full_name: name.into(),
            email: None,
            phone: None,
            file_reference: None,
        }
    }

    #[test]
    fn clients_round_trip_newest_first() {
        let mut s = Store::default();
        let a = s
            .client_save(None, &id("  Alice "), "{}", "2026-10-01T00:00:00Z")
            .unwrap();
        let b = s
            .client_save(None, &id("Bob"), "{}", "2026-10-02T00:00:00Z")
            .unwrap();
        assert_eq!((a, b), (1, 2));
        assert_eq!(s.client_get(a).unwrap().identity.full_name, "Alice");
        s.client_set_notes(a, vec!["à compléter".into()]).unwrap();
        s.client_save(Some(a), &id("Alice B"), "{\"x\":1}", "2026-10-03T00:00:00Z")
            .unwrap();
        assert_eq!(
            s.client_get(a).unwrap().notes,
            ["à compléter"],
            "saving keeps the notes"
        );
        let names: Vec<_> = s.client_list().into_iter().map(|c| c.full_name).collect();
        assert_eq!(names, ["Alice B", "Bob"]);
        assert!(s.client_delete(a));
        assert!(!s.client_delete(a));
        // Ids are never reused.
        assert_eq!(s.client_save(None, &id("C"), "{}", "t").unwrap(), 3);
    }

    #[test]
    fn client_validation() {
        let mut s = Store::default();
        assert!(s.client_save(None, &id(" "), "{}", "t").is_err());
        assert!(
            s.client_save(None, &id(&"x".repeat(201)), "{}", "t")
                .is_err()
        );
        assert!(s.client_save(Some(9), &id("A"), "{}", "t").is_err());
        let big = "x".repeat(MAX_PROFILE_BYTES + 1);
        assert!(s.client_save(None, &id("A"), &big, "t").is_err());
    }

    #[test]
    fn draws_are_inserted_once_and_listed_newest_first() {
        let mut s = Store::default();
        let d = |n: &str, date: &str| DrawRow {
            number: n.into(),
            date: date.into(),
            name: "x".into(),
            invitations: 1,
            min_crs: 500,
            first_seen: "t".into(),
        };
        assert_eq!(
            s.draws_upsert(&[d("9", "2026-01-01"), d("10", "2026-01-01")]),
            2
        );
        assert_eq!(
            s.draws_upsert(&[d("10", "2026-01-01"), d("11", "2026-02-01")]),
            1
        );
        let n: Vec<_> = s.draws_latest(2).into_iter().map(|d| d.number).collect();
        assert_eq!(n, ["11", "10"]);
    }

    #[test]
    fn last_run_and_last_ok() {
        let mut s = Store::default();
        let r = |at: &str, ok| WatchRun {
            at: at.into(),
            ok,
            summary: String::new(),
        };
        s.watch_run_record(&r("1", true));
        s.watch_run_record(&r("2", false));
        let (last, ok) = s.watch_runs_last();
        assert_eq!(last.unwrap().at, "2");
        assert_eq!(ok.unwrap().at, "1");
    }
}
