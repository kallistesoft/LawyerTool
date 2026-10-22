//! Client commands: list / get / save / delete, and assessment (CRS +
//! FSW / CEC / FST eligibility) against the grid in force on a date.

use std::sync::OnceLock;

use applicant::{Applicant, IsoDate};
use rules_engine::crs::CrsResult;
use rules_engine::eligibility::ProgramResult;
use rules_engine::grid::{Grid, in_force};
use serde::{Deserialize, Serialize};
use vault_file::{ClientSummary, DrawRow, Identity, Vault};

use crate::{CommandError, Res, err, time};

/// Approved grid snapshots shipped with the app (rulings applied).
fn grids() -> &'static Result<Vec<Grid>, String> {
    static GRIDS: OnceLock<Result<Vec<Grid>, String>> = OnceLock::new();
    GRIDS.get_or_init(|| {
        let raw = [
            include_str!("../../rules-engine/grids/crs-2026-10-16.json"),
            include_str!("../../rules-engine/grids/crs-2026-11-03.json"),
        ];
        raw.iter()
            .map(|json| {
                let mut g: Grid = serde_json::from_str(json).map_err(|e| format!("grid: {e}"))?;
                rules_engine::rulings::apply(&mut g);
                let issues = g.issues();
                if issues.is_empty() {
                    Ok(g)
                } else {
                    Err(format!(
                        "La grille {} comporte des lacunes : {}",
                        g.id,
                        issues
                            .iter()
                            .map(ToString::to_string)
                            .collect::<Vec<_>>()
                            .join(" ; ")
                    ))
                }
            })
            .collect()
    })
}

/// The grids shipped with the app (approved at build time).
pub fn shipped_grids() -> Result<&'static [Grid], String> {
    grids().as_ref().map(Vec::as_slice).map_err(Clone::clone)
}

/// One row of the client list: the key results at a glance.
#[derive(Debug, Serialize)]
pub struct ClientOverview {
    pub id: i64,
    pub full_name: String,
    pub file_reference: Option<String>,
    pub updated_at: String,
    /// CRS total on today's grid, or why it could not be computed.
    pub crs: Option<u16>,
    pub crs_error: Option<String>,
    pub programs: Vec<(
        rules_engine::eligibility::Program,
        rules_engine::eligibility::Status,
    )>,
    /// Mandatory lawyer review points.
    pub reviews: usize,
    /// Profile validation problems.
    pub problems: usize,
    /// Form import: estimated CRS and preliminary verdict.
    pub crs_estimate: Option<u16>,
    pub verdict: Option<crate::estimate::Verdict>,
}

/// Key results for one stored profile on `on` (pure; no vault access).
pub fn overview_of(
    c: ClientSummary,
    profile: &str,
    estimate: Option<&str>,
    grids: &[Grid],
    draws: &[DrawRow],
    on: &IsoDate,
) -> ClientOverview {
    let mut o = ClientOverview {
        id: c.id,
        full_name: c.full_name,
        file_reference: c.file_reference,
        updated_at: c.updated_at,
        crs: None,
        crs_error: None,
        programs: vec![],
        reviews: 0,
        problems: 0,
        crs_estimate: None,
        verdict: None,
    };
    if let (Some(s), Some(grid)) = (crate::estimate::stored(estimate), in_force(grids, on)) {
        let e = crate::estimate::evaluate(&s, grid, draws, on);
        o.crs_estimate = e.crs.as_ref().ok().map(|r| r.total);
        o.verdict = Some(e.verdict);
    }
    let Ok(a) = serde_json::from_str::<Applicant>(profile) else {
        o.crs_error = Some("Profil illisible.".into());
        return o;
    };
    o.problems = a.validate().err().map_or(0, |e| e.len());
    let Some(grid) = in_force(grids, on) else {
        o.crs_error = Some("Aucune grille officielle pour cette date.".into());
        return o;
    };
    match rules_engine::crs::score(grid, &a, on) {
        Ok(r) => {
            o.crs = Some(r.total);
            o.reviews = r.flags.iter().filter(|f| f.blocks_approval()).count();
        }
        Err(e) => o.crs_error = Some(e.to_string()),
    }
    if let Ok(ps) = rules_engine::eligibility::assess(grid, &a, on) {
        o.programs = ps.iter().map(|p| (p.program, p.status)).collect();
    }
    o
}

/// Draws read to find the latest of each category (the feed has a few a month).
const REFERENCE_DRAWS: usize = 60;

fn date(on: Option<String>) -> Res<IsoDate> {
    match on {
        Some(d) => {
            IsoDate::parse(&d).ok_or_else(|| err("invalid_date", "Date invalide (AAAA-MM-JJ)."))
        }
        None => Ok(time::today()),
    }
}

/// Every client with its score, eligibility and pending checks on `on`
/// (the UI's local date; default: today in UTC).
pub fn overview(v: &Vault, on: Option<String>) -> Res<Vec<ClientOverview>> {
    let on = date(on)?;
    let grids = crate::watch::all_grids(v)?;
    let draws = v.draws_latest(REFERENCE_DRAWS);
    Ok(v.client_list()
        .into_iter()
        .map(|c| {
            let r = v.client_get(c.id);
            let profile = r.as_ref().map(|r| r.profile.clone()).unwrap_or_default();
            let estimate = r.and_then(|r| r.estimate);
            overview_of(c, &profile, estimate.as_deref(), &grids, &draws, &on)
        })
        .collect())
}

#[derive(Serialize)]
pub struct ClientView {
    id: i64,
    created_at: String,
    updated_at: String,
    identity: Identity,
    profile: serde_json::Value,
    /// To complete or check (form import), until the lawyer clears them.
    notes: Vec<String>,
}

pub fn get(v: &Vault, id: i64) -> Res<ClientView> {
    let r = v
        .client_get(id)
        .ok_or_else(|| err("not_found", "Client introuvable."))?;
    let profile =
        serde_json::from_str(&r.profile).map_err(|_| err("corrupt", "Profil illisible."))?;
    Ok(ClientView {
        id: r.id,
        created_at: r.created_at,
        updated_at: r.updated_at,
        identity: r.identity,
        profile,
        notes: r.notes,
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

pub fn save(v: &mut Vault, request: SaveRequest) -> Res<Saved> {
    // The profile must match the Applicant model exactly (unknown fields are refused).
    let a: Applicant = serde_json::from_value(request.profile).map_err(|_| {
        err(
            "invalid_profile",
            "Profil invalide : données du formulaire mal formées.",
        )
    })?;
    let problems = a
        .validate()
        .err()
        .unwrap_or_default()
        .iter()
        .map(ToString::to_string)
        .collect();
    let json = serde_json::to_string(&a).map_err(|_| CommandError::internal())?;
    let id = v.client_save(request.id, &request.identity, &json, &time::now())?;
    Ok(Saved { id, problems })
}

#[derive(Serialize)]
pub struct Assessment {
    pub on: String,
    pub grid_id: Option<String>,
    /// The CRS score, or why it could not be computed.
    pub crs: Result<CrsResult, String>,
    pub eligibility: Result<Vec<ProgramResult>, String>,
    /// Latest official draws (context for the score; empty before the first watcher run).
    pub draws: Vec<DrawRow>,
    /// Form import: preliminary verdict from the client's answers.
    pub estimate: Option<crate::estimate::Estimated>,
}

/// Assesses a saved client on `on` (YYYY-MM-DD, default today).
pub fn assess(v: &Vault, id: i64, on: Option<String>) -> Res<Assessment> {
    let mut a = assess_inner(v, id, on)?;
    if let Ok(c) = &mut a.crs {
        localize(c);
    }
    if let Some(Ok(c)) = a.estimate.as_mut().map(|e| &mut e.crs) {
        localize(c);
    }
    Ok(a)
}

/// French labels for display (the export maps them itself).
fn localize(c: &mut CrsResult) {
    let sections = [
        &mut c.core,
        &mut c.spouse,
        &mut c.transferability,
        &mut c.additional,
    ];
    for s in sections.into_iter().chain(c.labour_market.as_mut()) {
        for l in &mut s.lines {
            l.label = crate::export::line_label(l.label);
        }
    }
    for f in &mut c.flags {
        if let rules_engine::crs::Flag::ExpiredLanguageTest { which } = f {
            *which = rules_engine::crs::language_fr(which);
        }
    }
}

pub fn assess_inner(v: &Vault, id: i64, on: Option<String>) -> Res<Assessment> {
    let on = date(on)?;
    let record = v
        .client_get(id)
        .ok_or_else(|| err("not_found", "Client introuvable."))?;
    let profile = record.profile;
    let grids = crate::watch::all_grids(v)?;
    let draws = v.draws_latest(5);
    let a: Applicant =
        serde_json::from_str(&profile).map_err(|_| err("corrupt", "Profil illisible."))?;
    let Some(grid) = in_force(&grids, &on) else {
        return Ok(Assessment {
            on: on.as_str().into(),
            grid_id: None,
            crs: Err("Aucune grille officielle pour cette date.".into()),
            eligibility: Err("Aucune grille officielle pour cette date.".into()),
            draws,
            estimate: None,
        });
    };
    let estimate = crate::estimate::stored(record.estimate.as_deref())
        .map(|s| crate::estimate::evaluate(&s, grid, &v.draws_latest(REFERENCE_DRAWS), &on));
    Ok(Assessment {
        on: on.as_str().into(),
        grid_id: Some(grid.id.clone()),
        crs: rules_engine::crs::score(grid, &a, &on).map_err(|e| e.to_string()),
        eligibility: rules_engine::eligibility::assess(grid, &a, &on).map_err(|e| e.to_string()),
        draws,
        estimate,
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

pub fn deid_preview(v: &Vault, id: i64, on: Option<String>) -> Res<Preview> {
    let assessment = assess_inner(v, id, on)?;
    let r = v
        .client_get(id)
        .ok_or_else(|| err("not_found", "Client introuvable."))?;
    let a: Applicant =
        serde_json::from_str(&r.profile).map_err(|_| err("corrupt", "Profil illisible."))?;
    let d = claude_payload(&r.identity, &a, &assessment);
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
        let g = rules_engine::grid::in_force(super::tests_grids(), &on)
            .unwrap_or_else(|| unreachable!());
        let assessment = super::Assessment {
            on: on.as_str().into(),
            grid_id: Some(g.id.clone()),
            crs: rules_engine::crs::score(g, &a, &on).map_err(|e| e.to_string()),
            eligibility: rules_engine::eligibility::assess(g, &a, &on).map_err(|e| e.to_string()),
            draws: vec![],
            estimate: None,
        };
        let identity = vault_file::Identity {
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
        for json in [
            include_str!("../tests/fixtures/ui-new-applicant.json"),
            include_str!("../tests/fixtures/ui-full-applicant.json"),
        ] {
            let a: applicant::Applicant =
                serde_json::from_str(json).unwrap_or_else(|e| panic!("UI profile rejected: {e}"));
            assert_eq!(a.validate(), Ok(()));
            let on = applicant::IsoDate::parse("2026-10-18").unwrap_or_else(|| unreachable!());
            let g = in_force(super::tests_grids(), &on).unwrap_or_else(|| unreachable!());
            let r = rules_engine::crs::score(g, &a, &on).unwrap_or_else(|e| panic!("{e}"));
            assert!(r.total > 0);
            assert_eq!(
                rules_engine::eligibility::assess(g, &a, &on).map(|v| v.len()),
                Ok(3)
            );
            assert!(serde_json::to_string(&r).is_ok());
        }
    }

    #[test]
    fn overview_shows_score_programs_and_pending_checks() {
        let grids = super::tests_grids();
        let on = applicant::IsoDate::parse("2026-10-18").unwrap_or_else(|| unreachable!());
        let row = |id| vault_file::ClientSummary {
            id,
            full_name: "X".into(),
            file_reference: None,
            updated_at: "t".into(),
        };
        let full = include_str!("../tests/fixtures/ui-full-applicant.json");
        let o = super::overview_of(row(1), full, None, grids, &[], &on);
        assert!(o.crs.is_some_and(|t| t > 0) && o.crs_error.is_none());
        assert_eq!(o.programs.len(), 3);
        assert_eq!(o.problems, 0);

        // Impossible profile: problems counted, score refused with a reason.
        let mut a: serde_json::Value = serde_json::from_str(full).unwrap_or_else(|e| panic!("{e}"));
        a["age"] = serde_json::json!(19);
        a["foreign_work_years"] = serde_json::json!(10);
        let o = super::overview_of(row(2), &a.to_string(), None, grids, &[], &on);
        assert!(o.problems > 0 && o.crs.is_none() && o.crs_error.is_some());

        // TCF at NCLC 4 relies on ruling D-001: one mandatory review.
        let mut a: serde_json::Value = serde_json::from_str(full).unwrap_or_else(|e| panic!("{e}"));
        a["first_language"] = serde_json::json!({
            "test": "tcf_canada", "test_date": "2026-01-15",
            "speaking": 4, "listening": 331, "reading": 342, "writing": 4
        });
        a["second_language"] = serde_json::Value::Null;
        let o = super::overview_of(row(3), &a.to_string(), None, grids, &[], &on);
        assert!(o.reviews >= 1, "{o:?}");

        let o = super::overview_of(row(4), "{", None, grids, &[], &on);
        assert_eq!(o.crs_error.as_deref(), Some("Profil illisible."));
    }
}
