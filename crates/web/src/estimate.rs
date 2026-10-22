//! Preliminary eligibility verdict for a client imported from the intake
//! form: can this person, from their own answers and in a cautious reading,
//! hope to immigrate through Express Entry?
//!
//! Three inputs, in this order of weight:
//! 1. show-stoppers declared in the form (a conviction, no usable language
//!    level): blocking whatever the score;
//! 2. program eligibility (FSW / CEC / FST) with the assumed language
//!    levels: blocking if no program remains possible;
//! 3. the estimated CRS score against the latest draw of each category the
//!    person can enter without knowing their occupation (French-language
//!    proficiency, Canadian experience, general).
//!
//! Anything the lawyer must look at (health, Québec, a score under the
//! cutoff…) makes the verdict "to examine". The official score and program
//! results still need real test results and the full profile.

use applicant::{Applicant, IsoDate};
use rules_engine::crs::{AssumedLevels, CrsResult};
use rules_engine::eligibility::{Check, Program, ProgramResult, Status};
use rules_engine::grid::Grid;
use serde::{Deserialize, Serialize};
use vault_file::DrawRow;

/// Lowest French level (NCLC, all skills) for the French-language draws.
const FRENCH_CATEGORY_MIN_NCLC: u8 = 7;
/// Draws older than this are not used as a reference (days).
const REFERENCE_MAX_AGE_DAYS: i32 = 180;

/// What a form import stores with the client (JSON in the vault).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stored {
    pub applicant: Applicant,
    pub levels: AssumedLevels,
    pub assumptions: Vec<String>,
    #[serde(default)]
    pub stoppers: Vec<String>,
    #[serde(default)]
    pub checks: Vec<String>,
    #[serde(default)]
    pub job_offer_declared: bool,
}

impl From<form_import::Estimate> for Stored {
    fn from(e: form_import::Estimate) -> Self {
        Self {
            applicant: e.applicant,
            levels: e.levels,
            assumptions: e.assumptions,
            stoppers: e.stoppers,
            checks: e.checks,
            job_offer_declared: e.job_offer_declared,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// A program seems possible and the estimated score reaches a recent cutoff.
    Favorable,
    /// Possible, but something must be examined first.
    ToExamine,
    /// A declared answer or the program rules stop the application.
    Blocking,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Blocking,
    Check,
    Positive,
}

#[derive(Debug, Clone, Serialize)]
pub struct Reason {
    pub kind: Kind,
    pub text: String,
}

#[derive(Debug, Serialize)]
pub struct Estimated {
    pub verdict: Verdict,
    /// Blocking reasons first, then checks, then positive points.
    pub reasons: Vec<Reason>,
    pub crs: Result<CrsResult, String>,
    pub eligibility: Result<Vec<ProgramResult>, String>,
    pub assumptions: Vec<String>,
}

/// The draw categories open to this person without an occupation code.
fn relevant(draw: &str, s: &Stored) -> bool {
    let n = draw.to_lowercase();
    if n.contains("français") || n.contains("francais") {
        s.levels
            .french
            .is_some_and(|l| l >= FRENCH_CATEGORY_MIN_NCLC)
    } else if n.contains("expérience canadienne") || n.contains("experience canadienne") {
        s.applicant.canadian_work_years >= 1
    } else {
        n.contains("aucun programme") || n.contains("général") || n.contains("general")
    }
}

/// The most recent draw of each relevant category, lowest cutoff first.
fn references<'a>(s: &Stored, draws: &'a [DrawRow], on: &IsoDate) -> Vec<&'a DrawRow> {
    let since = IsoDate::from_days(on.to_days() - REFERENCE_MAX_AGE_DAYS);
    let mut latest: Vec<&DrawRow> = Vec::new();
    // `draws` is newest first: keep the first draw seen per category name.
    for d in draws {
        let recent = since.as_ref().is_none_or(|s| d.date.as_str() >= s.as_str());
        if recent && relevant(&d.name, s) && !latest.iter().any(|x| x.name == d.name) {
            latest.push(d);
        }
    }
    latest.sort_by_key(|d| d.min_crs);
    latest
}

fn program_name(p: &ProgramResult) -> &'static str {
    crate::export::program_label(p.program)
}

/// Why the form's answers already rule a program out, when the engine can
/// only say "to complete" (no job history in an imported profile).
fn ruled_out_by_form(p: &ProgramResult, s: &Stored) -> Option<&'static str> {
    match p.program {
        Program::Cec if s.applicant.canadian_work_years == 0 => {
            Some("aucune expérience de travail au Canada déclarée")
        }
        Program::Fst if !s.job_offer_declared && !s.applicant.certificate_of_qualification => {
            Some("ni offre d’emploi ni certificat de compétence canadien déclarés")
        }
        _ => None,
    }
}

pub fn evaluate(s: &Stored, grid: &Grid, draws: &[DrawRow], on: &IsoDate) -> Estimated {
    let crs =
        rules_engine::crs::estimate(grid, &s.applicant, &s.levels, on).map_err(|e| e.to_string());
    let eligibility = rules_engine::eligibility::estimate(grid, &s.applicant, &s.levels, on)
        .map_err(|e| e.to_string());
    let mut reasons: Vec<Reason> = Vec::new();
    let mut add = |kind, text: String| reasons.push(Reason { kind, text });

    for t in &s.stoppers {
        add(Kind::Blocking, t.clone());
    }

    match &eligibility {
        Ok(programs) => {
            let possible: Vec<&ProgramResult> = programs
                .iter()
                .filter(|p| p.status != Status::NotEligible && ruled_out_by_form(p, s).is_none())
                .collect();
            if possible.is_empty() {
                let why: Vec<String> = programs
                    .iter()
                    .map(|p| {
                        let first = p.requirements.iter().find(|r| r.check == Check::NotMet);
                        format!(
                            "{} : {}",
                            program_name(p),
                            first
                                .map(|r| r.detail.as_str())
                                .or_else(|| ruled_out_by_form(p, s))
                                .unwrap_or("non admissible")
                        )
                    })
                    .collect();
                add(
                    Kind::Blocking,
                    format!(
                        "Aucun programme d’Entrée express possible ({}).",
                        why.join(" ; ")
                    ),
                );
            } else {
                let names: Vec<&str> = possible.iter().map(|p| program_name(p)).collect();
                add(
                    Kind::Positive,
                    format!(
                        "Programme(s) possible(s) : {}, sous réserve des renseignements à compléter (emplois, EDE, fonds).",
                        names.join(", ")
                    ),
                );
            }
        }
        Err(e) => add(
            Kind::Check,
            format!("Admissibilité aux programmes non évaluée : {e}."),
        ),
    }

    match &crs {
        Ok(r) => {
            let refs = references(s, draws, on);
            match refs.first() {
                Some(best) if r.total >= best.min_crs => add(
                    Kind::Positive,
                    format!(
                        "Score estimé {} ≥ seuil {} du dernier tirage « {} » ({}).",
                        r.total, best.min_crs, best.name, best.date
                    ),
                ),
                Some(best) => add(
                    Kind::Check,
                    format!(
                        "Score estimé {} sous le seuil le plus bas accessible : {} (« {} », {}), écart {}.",
                        r.total,
                        best.min_crs,
                        best.name,
                        best.date,
                        i32::from(r.total) - i32::from(best.min_crs)
                    ),
                ),
                None => add(
                    Kind::Check,
                    format!(
                        "Score estimé {} : aucun tirage récent de référence pour ce profil (vérifier la veille et les catégories liées au métier).",
                        r.total
                    ),
                ),
            }
        }
        // No usable language level is already a stopper.
        Err(_) if s.levels.french.is_none() && s.levels.english.is_none() => {}
        Err(e) => add(Kind::Check, format!("Score estimé non calculé : {e}.")),
    }

    for t in &s.checks {
        add(Kind::Check, t.clone());
    }

    let has = |k| reasons.iter().any(|r| r.kind == k);
    let verdict = if has(Kind::Blocking) {
        Verdict::Blocking
    } else if has(Kind::Check) {
        Verdict::ToExamine
    } else {
        Verdict::Favorable
    };
    reasons.sort_by_key(|r| match r.kind {
        Kind::Blocking => 0,
        Kind::Check => 1,
        Kind::Positive => 2,
    });
    Estimated {
        verdict,
        reasons,
        crs,
        eligibility,
        assumptions: s.assumptions.clone(),
    }
}

/// The stored estimate of a client, if it has one and it is readable.
pub fn stored(json: Option<&str>) -> Option<Stored> {
    serde_json::from_str(json?).ok()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    const SAMPLE: &[u8] = include_bytes!("../../form-import/tests/fixtures/sample-form.xlsx");

    fn on() -> IsoDate {
        IsoDate::parse("2026-10-21").unwrap()
    }

    fn sample() -> Vec<Stored> {
        form_import::parse(SAMPLE, "xlsx", &on())
            .unwrap()
            .responses
            .into_iter()
            .map(|r| Stored::from(r.estimate))
            .collect()
    }

    fn draw(date: &str, name: &str, min: u16) -> DrawRow {
        DrawRow {
            number: date.into(),
            date: date.into(),
            name: name.into(),
            invitations: 1000,
            min_crs: min,
            first_seen: "t".into(),
        }
    }

    fn grid() -> &'static Grid {
        let g = crate::clients::tests_grids();
        rules_engine::grid::in_force(g, &on()).unwrap()
    }

    #[test]
    fn french_speaker_without_issues_is_compared_with_the_french_draws() {
        let alix = &sample()[0];
        let draws = [
            draw(
                "2026-10-16",
                "Compétence linguistique en français 2026-Version 2",
                300,
            ),
            draw("2026-10-15", "Catégorie de l’expérience canadienne", 519),
            draw("2026-10-13", "Programme des candidats des provinces", 772),
        ];
        let e = evaluate(alix, grid(), &draws, &on());
        let total = e.crs.as_ref().unwrap().total;
        assert!(total >= 300, "{total}");
        assert_eq!(e.verdict, Verdict::Favorable, "{:#?}", e.reasons);
        assert!(
            e.reasons
                .iter()
                .any(|r| r.text.contains("Compétence linguistique en français"))
        );
        assert!(
            !e.reasons
                .iter()
                .any(|r| r.text.contains("expérience canadienne")),
            "no Canadian work"
        );

        // Cutoff above the estimate: to examine, with the gap.
        let high = [draw(
            "2026-10-16",
            "Compétence linguistique en français 2026-Version 2",
            900,
        )];
        let e = evaluate(alix, grid(), &high, &on());
        assert_eq!(e.verdict, Verdict::ToExamine);
        assert!(e.reasons.iter().any(|r| r.text.contains("écart -")));

        // Draws older than six months are ignored.
        let old = [draw(
            "2026-01-02",
            "Compétence linguistique en français 2026-Version 2",
            300,
        )];
        let e = evaluate(alix, grid(), &old, &on());
        assert!(
            e.reasons
                .iter()
                .any(|r| r.text.contains("aucun tirage récent"))
        );
    }

    #[test]
    fn declared_health_issue_means_to_examine_and_a_conviction_blocks() {
        let s = sample();
        let draws = [draw(
            "2026-10-16",
            "Compétence linguistique en français 2026-Version 2",
            300,
        )];
        let dominique = &s[2];
        assert_eq!(
            evaluate(dominique, grid(), &draws, &on()).verdict,
            Verdict::ToExamine
        );

        let mut convicted = s[0].clone();
        convicted
            .stoppers
            .push("Antécédent judiciaire déclaré « Condamné au Canada »".into());
        let e = evaluate(&convicted, grid(), &draws, &on());
        assert_eq!(e.verdict, Verdict::Blocking);
        assert_eq!(
            e.reasons[0].kind,
            Kind::Blocking,
            "blocking reasons come first"
        );
    }

    #[test]
    fn no_language_level_blocks_without_a_duplicate_reason() {
        let mut s = sample()[0].clone();
        s.levels = AssumedLevels::default();
        s.stoppers
            .push("Aucun niveau de langue suffisant déclaré".into());
        let e = evaluate(&s, grid(), &[], &on());
        assert_eq!(e.verdict, Verdict::Blocking);
        assert!(e.crs.is_err());
        assert!(
            !e.reasons
                .iter()
                .any(|r| r.text.starts_with("Score estimé non calculé"))
        );
    }
}
