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
    pub on: String,
    pub grid_id: Option<String>,
    /// The CRS score, or why it could not be computed.
    pub crs: Result<CrsResult, String>,
    pub eligibility: Result<Vec<ProgramResult>, String>,
}

/// Assesses a saved client on `on` (YYYY-MM-DD, default today).
#[tauri::command]
pub fn assess(
    s: State<'_, Arc<Session>>,
    id: i64,
    on: Option<String>,
) -> Result<Assessment, CommandError> {
    assess_inner(&s, id, on)
}

pub fn assess_inner(s: &Session, id: i64, on: Option<String>) -> Result<Assessment, CommandError> {
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

/// What would be sent to Claude to draft the note: the task, the profile and
/// the engine's results, de-identified. Nothing is sent by this command.
#[derive(Serialize)]
pub struct Preview {
    /// Exact JSON text that would be sent.
    payload: String,
    bytes: usize,
    redactions: Vec<deid::Redaction>,
}

pub fn claude_payload(
    identity: &Identity,
    a: &Applicant,
    assessment: &Assessment,
) -> deid::Deidentified {
    let raw = serde_json::json!({
        "task": "draft_explanatory_note",
        "client": deid::CLIENT,
        "assessed_on": assessment.on,
        "grid": assessment.grid_id,
        "profile": a,
        "crs": assessment.crs,
        "eligibility": assessment.eligibility,
    });
    let ids = deid::Identifiers {
        full_name: identity.full_name.clone(),
        email: identity.email.clone(),
        phone: identity.phone.clone(),
        file_reference: identity.file_reference.clone(),
    };
    deid::deidentify(raw, &ids)
}

#[tauri::command]
pub fn deid_preview(
    s: State<'_, Arc<Session>>,
    id: i64,
    on: Option<String>,
) -> Result<Preview, CommandError> {
    let assessment = assess_inner(&s, id, on)?;
    let (identity, profile) = s.with_vault(|v| {
        let r = v
            .client_get(id)?
            .ok_or_else(|| err("not_found", "Client introuvable."))?;
        Ok((r.identity, r.profile))
    })?;
    let a: Applicant =
        serde_json::from_str(&profile).map_err(|_| err("corrupt", "Profil illisible."))?;
    let d = claude_payload(&identity, &a, &assessment);
    let payload = serde_json::to_string_pretty(&d.payload).map_err(|_| CommandError::internal())?;
    Ok(Preview {
        bytes: payload.len(),
        payload,
        redactions: d.redactions,
    })
}

/// The shipped grids, for other modules' tests.
#[cfg(test)]
pub fn tests_grids() -> &'static [Grid] {
    grids()
        .as_ref()
        .map(Vec::as_slice)
        .unwrap_or_else(|e| panic!("{e}"))
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

    /// The Claude payload never contains the client's identity, even when the
    /// profile would (defence in depth).
    #[test]
    fn claude_payload_is_deidentified() {
        let a: applicant::Applicant =
            serde_json::from_str(include_str!("../tests/fixtures/ui-full-applicant.json"))
                .unwrap_or_else(|e| panic!("{e}"));
        let on = applicant::IsoDate::parse("2026-10-18").unwrap_or_else(|| unreachable!());
        let g = rules_engine::grid::in_force(
            super::grids().as_ref().unwrap_or_else(|e| panic!("{e}")),
            &on,
        )
        .unwrap_or_else(|| unreachable!());
        let assessment = super::Assessment {
            on: on.as_str().into(),
            grid_id: Some(g.id.clone()),
            crs: rules_engine::crs::score(g, &a, &on).map_err(|e| e.to_string()),
            eligibility: rules_engine::eligibility::assess(g, &a, &on).map_err(|e| e.to_string()),
        };
        let identity = vault::Identity {
            full_name: "Hélène Tremblay".into(),
            email: Some("h.t@example.com".into()),
            phone: Some("514-555-0142".into()),
            file_reference: Some("2026-041".into()),
        };
        let d = super::claude_payload(&identity, &a, &assessment);
        let text = d.payload.to_string();
        for leaked in [
            "Hélène",
            "Tremblay",
            "h.t@example.com",
            "555-0142",
            "2026-041",
        ] {
            assert!(!text.contains(leaked), "{leaked} leaked");
        }
        assert!(text.contains("\"client\":\"[CLIENT]\""));
        assert!(text.contains("\"profile\"") && text.contains("\"crs\""));
    }

    /// The TypeScript model (ui/src/lib/model.ts) must produce JSON the Rust
    /// Applicant accepts (unknown fields are refused). Fixtures are generated
    /// from the TS code; regenerate them when the model changes.
    #[test]
    fn ui_profiles_parse_and_assess() {
        use rules_engine::grid::in_force;
        let grids = super::grids().as_ref().unwrap_or_else(|e| panic!("{e}"));
        for json in [
            include_str!("../tests/fixtures/ui-new-applicant.json"),
            include_str!("../tests/fixtures/ui-full-applicant.json"),
        ] {
            let a: applicant::Applicant =
                serde_json::from_str(json).unwrap_or_else(|e| panic!("UI profile rejected: {e}"));
            assert_eq!(a.validate(), Ok(()));
            let on = applicant::IsoDate::parse("2026-10-18").unwrap_or_else(|| unreachable!());
            let g = in_force(grids, &on).unwrap_or_else(|| unreachable!());
            let r = rules_engine::crs::score(g, &a, &on).unwrap_or_else(|e| panic!("{e}"));
            assert!(r.total > 0);
            assert_eq!(
                rules_engine::eligibility::assess(g, &a, &on).map(|v| v.len()),
                Ok(3)
            );
            // The result must serialize for the UI.
            assert!(serde_json::to_string(&r).is_ok());
        }
    }

    #[test]
    fn today_is_a_date() {
        assert!(super::today().as_str() >= "2026-01-01");
    }
}
