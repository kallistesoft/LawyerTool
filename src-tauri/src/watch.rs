//! Rules watcher in the app: daily check while unlocked, pending change set,
//! lawyer approval into effective-dated grid snapshots, draws feed.
//!
//! The watcher crate fetches and compares public pages only; this module
//! stores its results in the vault (public data) and never passes client
//! data to it.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use rules_engine::grid::{Crs, EquivalencyTable, Grid, HighWageList, LabourMarket, in_force};
use rules_engine::programs::Programs;
use rules_watcher::{Change, Kind, SOURCES, SentenceChange};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter, Manager, State};
use vault::{DrawRow, SnapshotRow, Vault, WatchRun, WatchState};

use crate::clients::{shipped_grids, today};
use crate::session::{CommandError, Session};

/// Event sent to the window when a check finishes.
pub const WATCH_EVENT: &str = "watch-updated";
const DAY: u64 = 86_400;
const STALE_DAYS: u64 = 7;

/// Grid parts the watcher rebuilds (missing = that part failed to parse).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Parts {
    pub crs: Option<Crs>,
    pub language: Option<Vec<EquivalencyTable>>,
    pub programs: Option<Programs>,
    pub labour_market: Option<LabourMarket>,
    pub high_wage: Option<HighWageList>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProseChange {
    pub source: String,
    pub changes: Vec<SentenceChange>,
}

/// What the lawyer reviews.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pending {
    pub base_grid: String,
    pub changes: Vec<Change>,
    pub part_errors: Vec<String>,
    pub prose: Vec<ProseChange>,
    pub alerts: Vec<String>,
    pub candidate: Parts,
    pub hash: String,
}

fn now() -> String {
    audit::now_rfc3339()
}

fn ago(secs: u64) -> String {
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    audit::rfc3339(t.saturating_sub(secs))
}

fn verr(e: vault::Error) -> String {
    e.to_string()
}

/// Shipped grids plus lawyer-approved snapshots, rulings applied.
pub fn all_grids(v: &Vault) -> Result<Vec<Grid>, CommandError> {
    let mut grids = shipped_grids()
        .map_err(|e| CommandError::new("grids", e))?
        .to_vec();
    for s in v.snapshot_list()? {
        let mut g: Grid = serde_json::from_str(&s.grid)
            .map_err(|_| CommandError::new("corrupt", "Grille approuvée illisible."))?;
        rules_engine::rulings::apply(&mut g);
        grids.push(g);
    }
    Ok(grids)
}

fn latest(grids: &[Grid]) -> Option<&Grid> {
    grids
        .iter()
        .max_by(|a, b| a.effective_from.cmp(&b.effective_from))
}

/// The comparable parts of a grid (same shape as the candidate).
fn parts_of(g: &Grid) -> serde_json::Value {
    let mut crs = serde_json::to_value(&g.crs).unwrap_or_default();
    if let Some(o) = crs.as_object_mut() {
        o.remove("labour_market");
        o.remove("high_wage");
    }
    serde_json::json!({
        "crs": crs,
        "language": g.language,
        "programs": g.programs,
        "labour_market": g.crs.labour_market,
        "high_wage": g.crs.high_wage,
    })
}

/// Candidate parts in the same shape; parts that failed (or that the base
/// grid does not have) are taken from the base, so they show no change.
fn candidate_value(base: &Grid, c: &Parts) -> serde_json::Value {
    let mut v = parts_of(base);
    if let Some(crs) = &c.crs {
        v["crs"] = serde_json::to_value(crs).unwrap_or_default();
    }
    if let Some(l) = &c.language {
        v["language"] = serde_json::to_value(l).unwrap_or_default();
    }
    if let Some(p) = &c.programs {
        v["programs"] = serde_json::to_value(p).unwrap_or_default();
    }
    if base.crs.labour_market.is_some() {
        if let Some(lm) = &c.labour_market {
            v["labour_market"] = serde_json::to_value(lm).unwrap_or_default();
        }
        if let Some(h) = &c.high_wage {
            v["high_wage"] = serde_json::to_value(h).unwrap_or_default();
        }
    }
    v
}

fn apply_parts(g: &Grid, c: &Parts) -> Grid {
    let mut out = g.clone();
    if let Some(crs) = &c.crs {
        let (lm, hw) = (out.crs.labour_market.take(), out.crs.high_wage.take());
        out.crs = crs.clone();
        out.crs.labour_market = lm;
        out.crs.high_wage = hw;
    }
    if let Some(l) = &c.language {
        out.language = l.clone();
    }
    if let Some(p) = &c.programs {
        out.programs = Some(p.clone());
    }
    if g.crs.labour_market.is_some() {
        if let Some(lm) = &c.labour_market {
            out.crs.labour_market = Some(lm.clone());
        }
        if let Some(h) = &c.high_wage {
            out.crs.high_wage = Some(h.clone());
        }
    }
    out
}

#[derive(Debug, Serialize)]
pub struct RunSummary {
    pub at: String,
    pub sources_ok: usize,
    pub sources_failed: Vec<String>,
    pub changed_sources: Vec<String>,
    pub grid_changes: usize,
    pub new_draws: usize,
}

/// Keeps a parsed part, or records why it failed.
fn part<T>(r: Result<T, String>, errors: &mut Vec<String>) -> Option<T> {
    r.map_err(|e| errors.push(e)).ok()
}

static RUNNING: AtomicBool = AtomicBool::new(false);

/// One full check: fetch, fingerprint, candidate, diff, draws.
pub async fn run_check(app: &AppHandle) -> Result<RunSummary, String> {
    if RUNNING.swap(true, Ordering::SeqCst) {
        return Err("Une vérification est déjà en cours.".into());
    }
    let result = run_check_inner(app).await;
    RUNNING.store(false, Ordering::SeqCst);
    let _ = app.emit(WATCH_EVENT, ());
    result
}

async fn run_check_inner(app: &AppHandle) -> Result<RunSummary, String> {
    let session = app.state::<Arc<Session>>().inner().clone();
    if !session.is_unlocked() {
        return Err("Le coffre est verrouillé.".into());
    }
    let client = app.state::<net::Client>();
    let fetched = rules_watcher::fetch_all(&client).await;
    let at = now();

    let mut bodies: BTreeMap<&str, Vec<u8>> = BTreeMap::new();
    let mut failed = Vec::new();
    for f in fetched {
        match f.result {
            Ok(b) => {
                bodies.insert(f.id, b);
            }
            Err(e) => failed.push(format!("{} : {e}", f.id)),
        }
    }

    let summary = session
        .with_vault(|v| {
            let mut changed_sources = Vec::new();
            let mut prose = Vec::new();
            let mut alerts = Vec::new();
            for s in SOURCES {
                let Some(body) = bodies.get(s.id) else { continue };
                let fp = rules_watcher::fingerprint(s, body);
                let text = matches!(s.kind, Kind::Prose).then(|| rules_watcher::main_text(s, body)).flatten();
                let prev = v.watch_state_get(s.id)?;
                let changed = prev.as_ref().is_some_and(|p| p.fingerprint != fp);
                if changed {
                    changed_sources.push(s.id.to_owned());
                    match s.kind {
                        Kind::Prose => {
                            if let (Some(old), Some(new)) = (prev.as_ref().and_then(|p| p.main_text.clone()), text.clone()) {
                                prose.push(ProseChange { source: s.id.into(), changes: rules_watcher::sentence_diff(&old, &new) });
                            }
                        }
                        Kind::Watch => alerts.push(format!(
                            "{} a changé : refaire les profils de référence (calculateur IRCC) et vérifier le formulaire.",
                            s.id
                        )),
                        Kind::Grid | Kind::Draws => {}
                    }
                }
                let changed_at = match &prev {
                    Some(p) if !changed => p.changed_at.clone(),
                    _ => at.clone(),
                };
                v.watch_state_set(&WatchState { source_id: s.id.into(), fingerprint: fp, main_text: text, checked_at: at.clone(), changed_at })?;
            }

            // Draws.
            let mut new_draws = 0;
            if let Some(json) = bodies.get("ee-rounds") {
                match rules_watcher::parse_draws(json) {
                    Ok(d) => {
                        let rows: Vec<DrawRow> = d
                            .into_iter()
                            .map(|d| DrawRow { number: d.number, date: d.date, name: d.name, invitations: d.invitations, min_crs: d.min_crs, first_seen: at.clone() })
                            .collect();
                        new_draws = v.draws_upsert(&rows)?;
                    }
                    Err(e) => failed.push(e),
                }
            }

            // Candidate grid vs the latest approved grid.
            let grids = all_grids(v)?;
            let base = latest(&grids).ok_or_else(|| CommandError::new("grids", "Aucune grille."))?;
            let c = rules_watcher::build_candidate(&bodies);
            let mut part_errors = Vec::new();
            let parts = Parts {
                crs: part(c.crs, &mut part_errors),
                language: part(c.language, &mut part_errors),
                programs: part(c.programs, &mut part_errors),
                labour_market: part(c.labour_market, &mut part_errors),
                high_wage: part(c.high_wage, &mut part_errors),
            };
            let changes = rules_watcher::grid_changes(&parts_of(base), &candidate_value(base, &parts));
            let hash: String = Sha256::digest(serde_json::to_vec(&changes).unwrap_or_default()).iter().map(|b| format!("{b:02x}")).collect();
            let rejected = v.meta_get("watch_rejected_hash")?;
            let needs_review = (!changes.is_empty() && rejected.as_deref() != Some(hash.as_str())) || !prose.is_empty() || !alerts.is_empty() || !part_errors.is_empty();
            if needs_review {
                let p = Pending { base_grid: base.id.clone(), changes: changes.clone(), part_errors: part_errors.clone(), prose, alerts, candidate: parts, hash };
                v.watch_pending_set(&at, &serde_json::to_string(&p).map_err(|_| CommandError::internal())?)?;
            }

            let s = RunSummary {
                at: at.clone(),
                sources_ok: bodies.len(),
                sources_failed: failed.clone(),
                changed_sources,
                grid_changes: changes.len(),
                new_draws,
            };
            let ok_run = failed.is_empty() && part_errors.is_empty();
            v.watch_run_record(&WatchRun {
                at: at.clone(),
                ok: ok_run,
                summary: format!(
                    "{} sources lues, {} en échec, {} modification(s) de valeurs, {} nouveau(x) tirage(s)",
                    s.sources_ok,
                    s.sources_failed.len(),
                    s.grid_changes,
                    s.new_draws
                ),
            })?;
            Ok(s)
        })
        .map_err(|e| e.message().to_owned())?;
    Ok(summary)
}

/// Checks once a minute; runs when unlocked and the last successful check is
/// older than 24 h (so right after unlock, then daily while open).
pub fn spawn_daily(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(60)).await;
            let session = app.state::<Arc<Session>>().inner().clone();
            if !session.is_unlocked() {
                continue;
            }
            let due = session
                .with_vault(|v| {
                    Ok(v.watch_runs_last()
                        .map_err(verr)
                        .ok()
                        .and_then(|(_, ok)| ok)
                        .is_none_or(|r| r.at < ago(DAY)))
                })
                .unwrap_or(false);
            if due {
                let _ = run_check(&app).await;
            }
        }
    });
}

#[derive(Serialize)]
pub struct WatchStatus {
    last_run: Option<WatchRun>,
    last_ok: Option<WatchRun>,
    /// No successful check for more than 7 days.
    stale: bool,
    running: bool,
    pending: Option<(String, Pending)>,
    draws: Vec<DrawRow>,
    grids: Vec<(String, String)>,
}

#[tauri::command]
pub fn watch_status(s: State<'_, Arc<Session>>) -> Result<WatchStatus, CommandError> {
    s.with_vault(|v| {
        let (last_run, last_ok) = v.watch_runs_last()?;
        let stale = last_ok
            .as_ref()
            .is_none_or(|r| r.at < ago(STALE_DAYS * DAY));
        let pending = v
            .watch_pending_get()?
            .and_then(|(at, json)| serde_json::from_str::<Pending>(&json).ok().map(|p| (at, p)));
        let grids = all_grids(v)?
            .iter()
            .map(|g| (g.id.clone(), g.effective_from.as_str().to_owned()))
            .collect();
        Ok(WatchStatus {
            last_run,
            last_ok,
            stale,
            running: RUNNING.load(Ordering::SeqCst),
            pending,
            draws: v.draws_latest(10)?,
            grids,
        })
    })
}

#[tauri::command]
pub async fn watch_check_now(app: AppHandle) -> Result<RunSummary, CommandError> {
    run_check(&app)
        .await
        .map_err(|e| CommandError::new("watch_failed", e))
}

/// Approves the pending changes: new snapshots of the grid in force today and
/// of the latest grid, effective today (or from their own later date).
#[tauri::command]
pub fn watch_approve(
    s: State<'_, Arc<Session>>,
    note: Option<String>,
) -> Result<Vec<String>, CommandError> {
    s.with_vault(|v| approve(v, note, &today()))
}

/// Approval core (testable without Tauri).
pub fn approve(
    v: &Vault,
    note: Option<String>,
    today: &applicant::IsoDate,
) -> Result<Vec<String>, CommandError> {
    let today = today.clone();
    let (_, json) = v
        .watch_pending_get()?
        .ok_or_else(|| CommandError::new("nothing_pending", "Aucune modification en attente."))?;
    let p: Pending = serde_json::from_str(&json)
        .map_err(|_| CommandError::new("corrupt", "Modifications illisibles."))?;
    if p.changes.is_empty() {
        // Only alerts / prose / errors: acknowledging clears them.
        v.watch_pending_clear()?;
        return Ok(vec![]);
    }
    let grids = all_grids(v)?;
    let mut targets: Vec<&Grid> = Vec::new();
    for g in [in_force(&grids, &today), latest(&grids)]
        .into_iter()
        .flatten()
    {
        if !targets.iter().any(|t| t.id == g.id) {
            targets.push(g);
        }
    }
    let mut created = Vec::new();
    for g in targets {
        let mut new = apply_parts(g, &p.candidate);
        new.effective_from = std::cmp::max(today.clone(), g.effective_from.clone());
        let base_id = g.id.split("+watch").next().unwrap_or(&g.id).to_owned();
        new.id = format!("{base_id}+watch-{}", today.as_str());
        let mut n = 2;
        while grids.iter().any(|x| x.id == new.id) || created.contains(&new.id) {
            new.id = format!("{base_id}+watch-{}-{n}", today.as_str());
            n += 1;
        }
        let mut check = new.clone();
        rules_engine::rulings::apply(&mut check);
        let issues = check.issues();
        if !issues.is_empty() {
            return Err(CommandError::new(
                "grid_issues",
                format!("La grille candidate a des lacunes : {issues:?}"),
            ));
        }
        v.snapshot_save(&SnapshotRow {
            id: new.id.clone(),
            effective_from: new.effective_from.as_str().to_owned(),
            approved_at: now(),
            note: note.clone(),
            grid: serde_json::to_string(&new).map_err(|_| CommandError::internal())?,
        })?;
        created.push(new.id);
    }
    v.watch_pending_clear()?;
    Ok(created)
}

/// Rejects the pending changes; the same change set will not be proposed again.
#[tauri::command]
pub fn watch_reject(s: State<'_, Arc<Session>>) -> Result<(), CommandError> {
    s.with_vault(|v| {
        if let Some(p) = v
            .watch_pending_get()?
            .and_then(|(_, json)| serde_json::from_str::<Pending>(&json).ok())
        {
            v.meta_set("watch_rejected_hash", &p.hash)?;
        }
        v.watch_pending_clear()?;
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use applicant::IsoDate;

    use super::*;

    #[test]
    fn approval_creates_effective_dated_snapshots_used_by_assessments() {
        let dir = std::env::temp_dir().join(format!("lt-watch-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{e}"));
        let store = vault::MemoryKeyStore::default();
        let (v, _) = Vault::create(
            &vault::Paths::new(dir.join("t.db")),
            &store,
            "a long test passphrase",
            vault::KdfParams::INSECURE_FAST_FOR_TESTS,
        )
        .unwrap_or_else(|e| panic!("{e}"));

        let shipped = shipped_grids().unwrap_or_else(|e| panic!("{e}"));
        let mut crs = shipped[0].crs.clone();
        crs.age[1].points.with_spouse = 91; // 18 ans, avec époux: 90 -> 91
        let candidate = Parts {
            crs: Some(crs),
            ..Parts::default()
        };
        let changes = rules_watcher::grid_changes(
            &parts_of(&shipped[1]),
            &candidate_value(&shipped[1], &candidate),
        );
        assert_eq!(changes.len(), 1);
        let p = Pending {
            base_grid: shipped[1].id.clone(),
            changes,
            part_errors: vec![],
            prose: vec![],
            alerts: vec![],
            candidate,
            hash: "h".into(),
        };
        v.watch_pending_set("t", &serde_json::to_string(&p).unwrap_or_default())
            .unwrap_or_else(|e| panic!("{e}"));

        let today = IsoDate::parse("2026-10-20").unwrap_or_else(|| unreachable!());
        let created =
            approve(&v, Some("test".into()), &today).unwrap_or_else(|e| panic!("{}", e.message()));
        assert_eq!(
            created,
            vec![
                "crs-2026-10-16+watch-2026-10-20".to_owned(),
                "crs-2026-11-03+watch-2026-10-20".to_owned()
            ]
        );
        assert!(
            v.watch_pending_get()
                .unwrap_or_else(|e| panic!("{e}"))
                .is_none(),
            "pending cleared"
        );

        let grids = all_grids(&v).unwrap_or_else(|e| panic!("{}", e.message()));
        let now = in_force(&grids, &today).unwrap_or_else(|| unreachable!());
        assert_eq!(now.id, "crs-2026-10-16+watch-2026-10-20");
        assert_eq!(now.crs.age[1].points.with_spouse, 91);
        let later = in_force(
            &grids,
            &IsoDate::parse("2026-11-03").unwrap_or_else(|| unreachable!()),
        )
        .unwrap_or_else(|| unreachable!());
        assert_eq!(
            later.id, "crs-2026-11-03+watch-2026-10-20",
            "the future grid also gets the change"
        );
        assert_eq!(later.effective_from.as_str(), "2026-11-03");
        assert!(later.crs.labour_market.is_some());
        let before = in_force(
            &grids,
            &IsoDate::parse("2026-10-19").unwrap_or_else(|| unreachable!()),
        )
        .unwrap_or_else(|| unreachable!());
        assert_eq!(
            before.crs.age[1].points.with_spouse, 90,
            "past dates keep the old values"
        );

        // Nothing pending: approving again is refused.
        assert!(approve(&v, None, &today).is_err());
        drop(v);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn applying_candidate_parts_keeps_labour_market_where_it_belongs() {
        let grids = shipped_grids().unwrap_or_else(|e| panic!("{e}"));
        let (v0, v1) = (&grids[0], &grids[1]);
        let mut c = Parts {
            crs: Some(v0.crs.clone()),
            ..Parts::default()
        };
        if let Some(crs) = c.crs.as_mut() {
            crs.age[1].points.with_spouse = 91;
        }
        c.high_wage = v1.crs.high_wage.clone();
        let new0 = apply_parts(v0, &c);
        let new1 = apply_parts(v1, &c);
        assert_eq!(new0.crs.age[1].points.with_spouse, 91);
        assert!(
            new0.crs.labour_market.is_none(),
            "the current grid gains no labour market"
        );
        assert_eq!(new1.crs.age[1].points.with_spouse, 91);
        assert!(
            new1.crs.labour_market.is_some(),
            "the 2026-11-03 grid keeps its labour market"
        );
        let changes = rules_watcher::grid_changes(&parts_of(v1), &candidate_value(v1, &c));
        assert_eq!(changes.len(), 1, "{changes:#?}");
        assert_eq!(
            IsoDate::parse("2026-11-03").as_ref(),
            Some(&v1.effective_from)
        );
    }
}
