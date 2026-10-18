//! Client commands: list / get / save / delete, and assessment (CRS +
//! FSW / CEC / FST eligibility) against the grid in force on a date.

use std::sync::{Arc, OnceLock};

use applicant::{Applicant, IsoDate};
use rules_engine::crs::CrsResult;
use rules_engine::eligibility::ProgramResult;
use rules_engine::grid::{Grid, in_force};
use serde::{Deserialize, Serialize};
use tauri::State;
use vault::{ClientSummary, Identity};

use crate::session::{CommandError, Session};

/// Approved grid snapshots shipped with the app (rulings applied).
fn grids() -> &'static Result<Vec<Grid>, String> {
    static GRIDS: OnceLock<Result<Vec<Grid>, String>> = OnceLock::new();
    GRIDS.get_or_init(|| {
        let raw = [
            include_str!("../../crates/rules-engine/grids/crs-2026-10-16.json"),
            include_str!("../../crates/rules-engine/grids/crs-2026-11-03.json"),
        ];
        raw.iter()
            .map(|json| {
                let mut g: Grid = serde_json::from_str(json).map_err(|e| format!("grid: {e}"))?;
                rules_engine::rulings::apply(&mut g);
                let issues = g.issues();
                if issues.is_empty() {
                    Ok(g)
                } else {
                    Err(format!("grid {} has issues: {issues:?}", g.id))
                }
            })
            .collect()
    })
}

pub fn today() -> IsoDate {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let days = i32::try_from(secs / 86_400).unwrap_or(0);
    IsoDate::from_days(days)
        .unwrap_or_else(|| IsoDate::parse("1970-01-01").unwrap_or_else(|| unreachable!()))
}

fn err(code: &'static str, message: impl Into<String>) -> CommandError {
    CommandError::new(code, message)
}

#[tauri::command]
pub fn client_list(s: State<'_, Arc<Session>>) -> Result<Vec<ClientSummary>, CommandError> {
    s.with_vault(|v| Ok(v.client_list()?))
}

#[derive(Serialize)]
pub struct ClientView {
    id: i64,
    created_at: String,
    updated_at: String,
    identity: Identity,
    profile: serde_json::Value,
}

#[tauri::command]
pub fn client_get(s: State<'_, Arc<Session>>, id: i64) -> Result<ClientView, CommandError> {
    s.with_vault(|v| {
        let r = v
            .client_get(id)?
            .ok_or_else(|| err("not_found", "Client introuvable."))?;
        let profile =
            serde_json::from_str(&r.profile).map_err(|_| err("corrupt", "Profil illisible."))?;
        Ok(ClientView {
            id: r.id,
            created_at: r.created_at,
            updated_at: r.updated_at,
            identity: r.identity,
            profile,
        })
    })
}

#[derive(Deserialize)]
pub struct SaveRequest {
    id: Option<i64>,
    identity: Identity,
    profile: serde_json::Value,
}

#[derive(Serialize)]
pub struct Saved {
    id: i64,
    /// Problems found in the profile (saved anyway, as a draft).
    problems: Vec<String>,
}

#[tauri::command]
pub fn client_save(
    s: State<'_, Arc<Session>>,
    request: SaveRequest,
) -> Result<Saved, CommandError> {
    // The profile must match the Applicant model exactly (unknown fields are refused).
    let a: Applicant = serde_json::from_value(request.profile.clone())
        .map_err(|e| err("invalid_profile", format!("Profil invalide : {e}")))?;
    let problems = a
        .validate()
        .err()
        .unwrap_or_default()
        .iter()
        .map(ToString::to_string)
        .collect();
    let json = serde_json::to_string(&a).map_err(|_| CommandError::internal())?;
    s.with_vault(|v| {
        let id = v.client_save(request.id, &request.identity, &json, &audit::now_rfc3339())?;
        Ok(Saved { id, problems })
    })
}

#[tauri::command]
pub fn client_delete(s: State<'_, Arc<Session>>, id: i64) -> Result<bool, CommandError> {
    s.with_vault(|v| Ok(v.client_delete(id)?))
}

#[derive(Serialize)]
pub struct Assessment {
    on: String,
    grid_id: Option<String>,
    /// The CRS score, or why it could not be computed.
    crs: Result<CrsResult, String>,
    eligibility: Result<Vec<ProgramResult>, String>,
}

/// Assesses a saved client on `on` (YYYY-MM-DD, default today).
#[tauri::command]
pub fn assess(
    s: State<'_, Arc<Session>>,
    id: i64,
    on: Option<String>,
) -> Result<Assessment, CommandError> {
    let on = match on {
        Some(d) => {
            IsoDate::parse(&d).ok_or_else(|| err("invalid_date", "Date invalide (AAAA-MM-JJ)."))?
        }
        None => today(),
    };
    let grids = grids().as_ref().map_err(|e| err("grids", e.clone()))?;
    let profile = s.with_vault(|v| {
        Ok(v.client_get(id)?
            .ok_or_else(|| err("not_found", "Client introuvable."))?
            .profile)
    })?;
    let a: Applicant =
        serde_json::from_str(&profile).map_err(|_| err("corrupt", "Profil illisible."))?;
    let Some(grid) = in_force(grids, &on) else {
        return Ok(Assessment {
            on: on.as_str().into(),
            grid_id: None,
            crs: Err("Aucune grille officielle pour cette date.".into()),
            eligibility: Err("Aucune grille officielle pour cette date.".into()),
        });
    };
    Ok(Assessment {
        on: on.as_str().into(),
        grid_id: Some(grid.id.clone()),
        crs: rules_engine::crs::score(grid, &a, &on).map_err(|e| e.to_string()),
        eligibility: rules_engine::eligibility::assess(grid, &a, &on).map_err(|e| e.to_string()),
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn shipped_grids_load_without_issues() {
        let g = super::grids().as_ref().unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(g.len(), 2);
        assert!(
            g.iter()
                .all(|g| g.programs.is_some() && !g.language.is_empty())
        );
    }

    #[test]
    fn today_is_a_date() {
        assert!(super::today().as_str() >= "2026-01-01");
    }
}
