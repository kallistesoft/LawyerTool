//! Express Entry program eligibility: FSW, CEC, FST minimum requirements and
//! the FSW 67-point selection grid. Never guesses: a missing fact makes the
//! requirement "needs input"; facts only a lawyer can judge are listed as
//! "lawyer check".

use applicant::{
    Applicant, Credential, EducationLevel, IsoDate, LanguageTest, Skill, ValidationError,
    WorkPeriod,
};
use serde::Serialize;

use crate::grid::{Cite, Grid};
use crate::language::{Clb, to_clb, valid_on};
use crate::programs::{ExperienceRule, Fsw, Funds, Programs};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Program {
    Fsw,
    Cec,
    Fst,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Check {
    Met,
    NotMet,
    NeedsInput,
    /// Cannot be computed; the lawyer must confirm it.
    LawyerCheck,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Requirement {
    pub id: &'static str,
    pub check: Check,
    pub detail: String,
    pub cites: Vec<Cite>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Status {
    /// Every computable requirement is met; lawyer checks remain to confirm.
    Eligible,
    NotEligible,
    NeedsInput,
}

/// FSW selection points as a range: unknown facts count 0 in `min` and their
/// full value in `max`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FswPoints {
    pub lines: Vec<(&'static str, u16, u16)>,
    pub min: u16,
    pub max: u16,
    pub pass_mark: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProgramResult {
    pub program: Program,
    pub status: Status,
    pub requirements: Vec<Requirement>,
    pub fsw_points: Option<FswPoints>,
}

impl ProgramResult {
    pub fn requirement(&self, id: &str) -> Option<&Requirement> {
        self.requirements.iter().find(|r| r.id == id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EligibilityError {
    #[error("profil du demandeur invalide : {}", crate::crs::join(.0))]
    Invalid(Vec<ValidationError>),
    #[error("la grille {0} ne contient pas les règles des programmes")]
    NoProgramRules(String),
}

fn req(id: &'static str, check: Check, detail: impl Into<String>, cites: Vec<Cite>) -> Requirement {
    Requirement {
        id,
        check,
        detail: detail.into(),
        cites,
    }
}

fn status(reqs: &[Requirement]) -> Status {
    if reqs.iter().any(|r| r.check == Check::NotMet) {
        Status::NotEligible
    } else if reqs.iter().any(|r| r.check == Check::NeedsInput) {
        Status::NeedsInput
    } else {
        Status::Eligible
    }
}

/// Hours worked from `from` to `to` inclusive, counting at most `cap` hours
/// per week across concurrent jobs.
pub fn hours<'a>(
    periods: impl IntoIterator<Item = &'a WorkPeriod>,
    from: &IsoDate,
    to: &IsoDate,
    cap: u16,
) -> u32 {
    let spans: Vec<(i32, i32, u16)> = periods
        .into_iter()
        .map(|p| {
            (
                p.start.to_days(),
                p.end.to_days(),
                u16::from(p.hours_per_week),
            )
        })
        .collect();
    let (Some(lo), Some(hi)) = (
        spans.iter().map(|s| s.0).min(),
        spans.iter().map(|s| s.1).max(),
    ) else {
        return 0;
    };
    let (from, to) = (from.to_days().max(lo), to.to_days().min(hi));
    // Sum weekly hours per day in sevenths of an hour, then convert.
    let sevenths: u64 = (from..=to)
        .map(|d| {
            let weekly: u16 = spans
                .iter()
                .filter(|(s, e, _)| *s <= d && d <= *e)
                .map(|s| s.2)
                .sum();
            u64::from(weekly.min(cap))
        })
        .sum();
    u32::try_from(sevenths / 7).unwrap_or(u32::MAX)
}

/// First day of an `years`-year window ending on `on`.
fn window_start(on: &IsoDate, years: u16) -> IsoDate {
    on.minus_years(years).next_day()
}

fn exp_cites(r: &ExperienceRule) -> Vec<Cite> {
    vec![
        r.min_hours.cite.clone(),
        r.window_years.cite.clone(),
        r.max_hours_per_week.cite.clone(),
    ]
}

/// Best single-NOC hours among `periods` in the window: (noc, hours).
fn best_noc<'a>(
    periods: &[&'a WorkPeriod],
    rule: &ExperienceRule,
    on: &IsoDate,
) -> Option<(&'a str, u32)> {
    let from = window_start(on, rule.window_years.value);
    let mut nocs: Vec<&str> = periods.iter().map(|p| p.noc.as_str()).collect();
    nocs.sort_unstable();
    nocs.dedup();
    nocs.into_iter()
        .map(|n| {
            (
                n,
                hours(
                    periods.iter().copied().filter(|p| p.noc.as_str() == n),
                    &from,
                    on,
                    rule.max_hours_per_week.value,
                ),
            )
        })
        .max_by_key(|(_, h)| *h)
}

fn levels(grid: &Grid, t: &LanguageTest) -> Option<Clb> {
    to_clb(grid, t).ok().map(|(c, _)| c)
}

/// First official language result, if valid on `on`.
fn first_language(grid: &Grid, a: &Applicant, on: &IsoDate) -> Result<Clb, Requirement> {
    let Some(t) = &a.first_language else {
        return Err(req(
            "language",
            Check::NotMet,
            "aucun test de langue reconnu",
            vec![],
        ));
    };
    if !valid_on(t, on) {
        return Err(req(
            "language",
            Check::NotMet,
            format!(
                "le test du {} n’est plus valide le {}",
                t.test_date.as_str(),
                on.as_str()
            ),
            vec![],
        ));
    }
    levels(grid, t).ok_or_else(|| {
        req(
            "language",
            Check::NeedsInput,
            "le résultat du test de langue n’a pas pu être converti en NCLC",
            vec![],
        )
    })
}

fn quebec(a: &Applicant) -> Requirement {
    match a.eligibility.intends_to_live_in_quebec {
        Some(true) => req(
            "outside_quebec",
            Check::NotMet,
            "compte s’établir au Québec (le Québec sélectionne ses propres travailleurs)",
            vec![],
        ),
        Some(false) => req(
            "outside_quebec",
            Check::Met,
            "compte s’établir hors du Québec",
            vec![],
        ),
        None => req(
            "outside_quebec",
            Check::NeedsInput,
            "province de destination inconnue",
            vec![],
        ),
    }
}

fn admissibility() -> Requirement {
    req(
        "admissible",
        Check::LawyerCheck,
        "admissibilité au Canada (sécurité, criminalité, santé)",
        vec![],
    )
}

fn duties() -> Requirement {
    req(
        "noc_duties",
        Check::LawyerCheck,
        "les fonctions correspondent à l’énoncé principal et à la plupart des fonctions principales de la CNP pour chaque emploi compté",
        vec![],
    )
}

fn funds(f: &Funds, a: &Applicant) -> Requirement {
    let e = &a.eligibility;
    let exempt = e.authorized_to_work_in_canada == Some(true) && a.job_offer.is_some();
    if exempt {
        return req(
            "funds",
            Check::Met,
            "dispensé : autorisé à travailler au Canada avec une offre d’emploi valide",
            vec![],
        );
    }
    let (Some(size), Some(have)) = (e.family_size, e.settlement_funds_cad) else {
        return req(
            "funds",
            Check::NeedsInput,
            "taille de la famille et fonds d’établissement requis",
            vec![],
        );
    };
    let Some((need, mut cites)) = f.required(size) else {
        return req(
            "funds",
            Check::NeedsInput,
            "taille de la famille hors du tableau",
            vec![],
        );
    };
    cites.push(Cite {
        source: "preuve-fonds".into(),
        table: None,
        row: None,
        text: format!("Mise à jour : le {}", f.updated),
    });
    let maybe_exempt = e.authorized_to_work_in_canada.is_none() && a.job_offer.is_some();
    match (have >= need, maybe_exempt) {
        (true, _) => req(
            "funds",
            Check::Met,
            format!("{have} $ ≥ {need} $ requis pour {size} personne(s)"),
            cites,
        ),
        (false, true) => req(
            "funds",
            Check::NeedsInput,
            format!("{have} $ < {need} $ ; dispensé si autorisé à travailler au Canada"),
            cites,
        ),
        (false, false) => req(
            "funds",
            Check::NotMet,
            format!("{have} $ < {need} $ requis pour {size} personne(s)"),
            cites,
        ),
    }
}

/// (min, max) points for one adaptability or arranged-employment item.
fn known(value: Option<bool>, points: u16) -> (u16, u16) {
    match value {
        Some(true) => (points, points),
        Some(false) => (0, 0),
        None => (0, points),
    }
}

fn fsw_points(
    grid: &Grid,
    f: &Fsw,
    a: &Applicant,
    on: &IsoDate,
    fol: Option<&Clb>,
    skilled_years: Option<u8>,
) -> FswPoints {
    let mut lines: Vec<(&'static str, u16, u16)> = Vec::new();
    let both = |p: u16| (p, p);

    let lang: u16 = fol.map_or(0, |c| {
        Skill::ALL
            .iter()
            .map(|s| {
                f.first_language
                    .iter()
                    .find(|r| r.key.contains(c.get(*s)))
                    .map_or(0, |r| r.points)
            })
            .sum()
    });
    lines.push(("first official language", lang, lang));
    let sol = a
        .second_language
        .as_ref()
        .filter(|t| valid_on(t, on))
        .and_then(|t| levels(grid, t))
        .is_some_and(|c| c.all_at_least(f.second_language_min));
    let (lo, hi) = both(if sol { f.second_language.points } else { 0 });
    lines.push(("second official language", lo, hi));

    let edu = f
        .education
        .iter()
        .find(|r| r.key == a.education)
        .map_or(0, |r| r.points);
    lines.push(("education", edu, edu));

    let (lo, hi) = match skilled_years {
        Some(y) => both(
            f.work_experience
                .iter()
                .find(|r| r.key.contains(y))
                .map_or(0, |r| r.points),
        ),
        None => (
            0,
            f.work_experience
                .iter()
                .map(|r| r.points)
                .max()
                .unwrap_or(0),
        ),
    };
    lines.push(("work experience", lo, hi));

    let age = f
        .age
        .iter()
        .find(|r| r.key.contains(a.age))
        .map_or(0, |r| r.points);
    lines.push(("age", age, age));

    let e = &a.eligibility;
    let (lo, hi) = known(e.fsw_arranged_employment, f.arranged_employment.value);
    lines.push(("arranged employment", lo, hi));

    // Adaptability (capped).
    let ad = &f.adaptability;
    let with_spouse = a.scored_with_spouse();
    let spouse = a.spouse.as_ref().filter(|_| with_spouse);
    let own_work = {
        let canadian: Vec<&WorkPeriod> = e
            .work_history
            .iter()
            .filter(|p| {
                p.in_canada
                    && p.authorized
                    && p.paid
                    && p.noc.teer() <= 3
                    && u16::from(p.hours_per_week) >= f.experience.max_hours_per_week.value
            })
            .collect();
        if e.work_history.is_empty() {
            None
        } else {
            let all = hours(
                canadian,
                &IsoDate::from_days(0).unwrap_or_else(|| on.clone()),
                on,
                f.experience.max_hours_per_week.value,
            );
            Some(all >= u32::from(f.experience.min_hours.value))
        }
    };
    let items = [
        known(
            spouse
                .map(|s| {
                    s.language
                        .as_ref()
                        .filter(|t| valid_on(t, on))
                        .and_then(|t| levels(grid, t))
                        .is_some_and(|c| c.all_at_least(ad.spouse_language_min))
                })
                .or(Some(false)), // no accompanying spouse
            ad.spouse_language.points,
        ),
        known(e.canadian_study_two_years, ad.own_canadian_study.points),
        known(
            spouse
                .and(e.spouse_canadian_study_two_years)
                .or(spouse.is_none().then_some(false)),
            ad.spouse_canadian_study.points,
        ),
        known(own_work, ad.own_canadian_work.points),
        known(
            spouse
                .map(|s| {
                    e.spouse_canadian_work_one_year
                        .or((s.canadian_work_years >= 1).then_some(true))
                })
                .unwrap_or(Some(false)),
            ad.spouse_canadian_work.points,
        ),
        known(e.fsw_arranged_employment, ad.arranged_employment.points),
        known(
            e.relative_in_canada.or(a.sibling_in_canada.then_some(true)),
            ad.relative_in_canada.points,
        ),
    ];
    let lo: u16 = items.iter().map(|i| i.0).sum();
    let hi: u16 = items.iter().map(|i| i.1).sum();
    lines.push(("adaptability", lo.min(ad.max.value), hi.min(ad.max.value)));

    FswPoints {
        min: lines.iter().map(|l| l.1).sum(),
        max: lines.iter().map(|l| l.2).sum(),
        lines,
        pass_mark: f.pass_mark.value,
    }
}

fn fsw(grid: &Grid, p: &Programs, a: &Applicant, on: &IsoDate) -> ProgramResult {
    let f = &p.fsw;
    let e = &a.eligibility;
    let mut reqs = Vec::new();

    // Experience: 1 year (1,560 h) in one TEER 0–3 NOC within 10 years.
    let skilled: Vec<&WorkPeriod> = e
        .work_history
        .iter()
        .filter(|w| w.paid && w.noc.teer() <= 3)
        .collect();
    let mut skilled_years = None;
    if e.work_history.is_empty() {
        reqs.push(req(
            "experience",
            Check::NeedsInput,
            "antécédents de travail requis",
            exp_cites(&f.experience),
        ));
    } else {
        let min = u32::from(f.experience.min_hours.value);
        let from = window_start(on, f.experience.window_years.value);
        let total = hours(
            skilled.iter().copied(),
            &from,
            on,
            f.experience.max_hours_per_week.value,
        );
        skilled_years = Some(u8::try_from(total / min).unwrap_or(u8::MAX));
        match best_noc(&skilled, &f.experience, on) {
            Some((noc, h)) if h >= min => reqs.push(req(
                "experience",
                Check::Met,
                format!(
                    "{h} h dans la CNP {noc} au cours des {} dernières années",
                    f.experience.window_years.value
                ),
                exp_cites(&f.experience),
            )),
            best => reqs.push(req(
                "experience",
                Check::NotMet,
                format!(
                    "meilleure profession FEER 0–3 : {} h ({min} h requises)",
                    best.map_or(0, |b| b.1)
                ),
                exp_cites(&f.experience),
            )),
        }
        reqs.push(duties());
    }

    let fol = match first_language(grid, a, on) {
        Ok(c) if c.all_at_least(f.min_clb.value as u8) => {
            reqs.push(req(
                "language",
                Check::Met,
                format!(
                    "NCLC {} ou plus dans toutes les compétences",
                    f.min_clb.value
                ),
                vec![f.min_clb.cite.clone()],
            ));
            Some(c)
        }
        Ok(c) => {
            reqs.push(req(
                "language",
                Check::NotMet,
                format!(
                    "compétence la plus faible : NCLC {} ({} requis)",
                    c.min(),
                    f.min_clb.value
                ),
                vec![f.min_clb.cite.clone()],
            ));
            Some(c)
        }
        Err(r) => {
            reqs.push(r);
            None
        }
    };

    reqs.push(match (a.education, e.credential) {
        (EducationLevel::LessThanSecondary, _) => req(
            "education",
            Check::NotMet,
            "diplôme d’études secondaires ou supérieur requis",
            vec![],
        ),
        (_, Some(Credential::Canadian | Credential::ForeignWithEca)) => req(
            "education",
            Check::Met,
            "diplôme canadien ou diplôme étranger avec une évaluation (EDE)",
            vec![],
        ),
        (_, Some(Credential::ForeignWithoutEca)) => req(
            "education",
            Check::NotMet,
            "le diplôme étranger doit faire l’objet d’une évaluation des diplômes d’études (EDE)",
            vec![],
        ),
        (_, None) => req(
            "education",
            Check::NeedsInput,
            "type de diplôme requis (canadien / étranger avec EDE)",
            vec![],
        ),
    });

    let points = fsw_points(grid, f, a, on, fol.as_ref(), skilled_years);
    let cites = vec![f.pass_mark.cite.clone()];
    reqs.push(if points.min >= points.pass_mark {
        req(
            "selection_points",
            Check::Met,
            format!(
                "{}–{} / 100 (note de passage {})",
                points.min, points.max, points.pass_mark
            ),
            cites,
        )
    } else if points.max < points.pass_mark {
        req(
            "selection_points",
            Check::NotMet,
            format!(
                "au plus {} / 100 (note de passage {})",
                points.max, points.pass_mark
            ),
            cites,
        )
    } else {
        req(
            "selection_points",
            Check::NeedsInput,
            format!(
                "{}–{} / 100 selon les renseignements manquants (note de passage {})",
                points.min, points.max, points.pass_mark
            ),
            cites,
        )
    });

    reqs.push(funds(&p.funds, a));
    reqs.push(quebec(a));
    reqs.push(admissibility());
    ProgramResult {
        program: Program::Fsw,
        status: status(&reqs),
        requirements: reqs,
        fsw_points: Some(points),
    }
}

fn cec(grid: &Grid, p: &Programs, a: &Applicant, on: &IsoDate) -> ProgramResult {
    let c = &p.cec;
    let e = &a.eligibility;
    let mut reqs = Vec::new();
    let rule = &c.experience;
    let from = window_start(on, rule.window_years.value);
    let cap = rule.max_hours_per_week.value;
    let min = u32::from(rule.min_hours.value);
    let counted: Vec<&WorkPeriod> = e
        .work_history
        .iter()
        .filter(|w| {
            w.in_canada
                && w.authorized
                && w.paid
                && !w.self_employed
                && !w.full_time_student
                && w.noc.teer() <= 3
        })
        .collect();
    let total = hours(counted.iter().copied(), &from, on, cap);
    let teer23 = hours(
        counted.iter().copied().filter(|w| w.noc.teer() >= 2),
        &from,
        on,
        cap,
    );

    if e.work_history.is_empty() {
        reqs.push(req(
            "experience",
            Check::NeedsInput,
            "antécédents de travail requis",
            exp_cites(rule),
        ));
    } else {
        let check = if total >= min {
            Check::Met
        } else {
            Check::NotMet
        };
        reqs.push(req(
            "experience",
            check,
            format!("{total} h de travail autorisé et rémunéré FEER 0–3 au Canada au cours des {} dernières années ({min} h requises ; travail autonome et travail pendant des études à temps plein exclus)", rule.window_years.value),
            exp_cites(rule),
        ));
        reqs.push(duties());
    }

    let (hi, lo) = (
        c.min_clb_teer_0_1.value as u8,
        c.min_clb_teer_2_3.value as u8,
    );
    let cites = vec![
        c.min_clb_teer_0_1.cite.clone(),
        c.min_clb_teer_2_3.cite.clone(),
    ];
    reqs.push(match first_language(grid, a, on) {
        Err(r) => r,
        Ok(l) if l.all_at_least(hi) => req("language", Check::Met, format!("NCLC {hi} ou plus dans toutes les compétences"), cites),
        Ok(l) if l.all_at_least(lo) && teer23 >= min => {
            req("language", Check::Met, format!("NCLC {lo} ou plus avec une expérience FEER 2–3"), cites)
        }
        Ok(l) if l.all_at_least(lo) && total >= min && teer23 > 0 => req(
            "language",
            Check::LawyerCheck,
            format!("NCLC {} : suffisant pour la CEC seulement avec une expérience FEER 2–3 ; l’expérience admissible combine FEER 0–1 et 2–3", l.min()),
            cites,
        ),
        Ok(l) => req("language", Check::NotMet, format!("compétence la plus faible : NCLC {} ({lo} requis pour FEER 2–3, {hi} pour FEER 0–1)", l.min()), cites),
    });
    reqs.push(quebec(a));
    reqs.push(admissibility());
    ProgramResult {
        program: Program::Cec,
        status: status(&reqs),
        requirements: reqs,
        fsw_points: None,
    }
}

fn fst(grid: &Grid, p: &Programs, a: &Applicant, on: &IsoDate) -> ProgramResult {
    let t = &p.fst;
    let e = &a.eligibility;
    let mut reqs = Vec::new();
    let min = u32::from(t.experience.min_hours.value);
    let trade = |w: &&WorkPeriod| t.trades.iter().any(|g| g.contains(w.noc.as_str()));
    let counted: Vec<&WorkPeriod> = e
        .work_history
        .iter()
        .filter(|w| w.paid && !w.full_time_student && w.qualified_in_trade != Some(false))
        .filter(trade)
        .collect();
    let mut cites = exp_cites(&t.experience);
    cites.push(t.trades_cite.clone());
    if e.work_history.is_empty() {
        reqs.push(req(
            "experience",
            Check::NeedsInput,
            "antécédents de travail requis",
            cites,
        ));
    } else {
        match best_noc(&counted, &t.experience, on) {
            Some((noc, h)) if h >= min => {
                reqs.push(req(
                    "experience",
                    Check::Met,
                    format!(
                        "{h} h dans le métier spécialisé CNP {noc} au cours des {} dernières années",
                        t.experience.window_years.value
                    ),
                    cites,
                ));
                if counted
                    .iter()
                    .any(|w| w.noc.as_str() == noc && w.qualified_in_trade.is_none())
                {
                    reqs.push(req("qualified_in_trade", Check::LawyerCheck, "confirmer que le demandeur était qualifié pour exercer le métier là où le travail a été effectué", vec![]));
                }
            }
            best => reqs.push(req(
                "experience",
                Check::NotMet,
                format!(
                    "meilleur métier spécialisé : {} h ({min} h requises dans une même CNP)",
                    best.map_or(0, |b| b.1)
                ),
                cites,
            )),
        }
        reqs.push(duties());
    }

    reqs.push(if a.job_offer.is_some() || a.certificate_of_qualification {
        req("offer_or_certificate", Check::Met, "offre d’emploi valide ou certificat de compétence", vec![])
    } else {
        req("offer_or_certificate", Check::NotMet, "offre d’emploi à temps plein valide (1 an ou plus) ou certificat de compétence canadien dans le métier requis", vec![])
    });

    let (sl, rw) = (
        t.min_clb_speaking_listening.value as u8,
        t.min_clb_reading_writing.value as u8,
    );
    let cites = vec![
        t.min_clb_speaking_listening.cite.clone(),
        t.min_clb_reading_writing.cite.clone(),
    ];
    reqs.push(match first_language(grid, a, on) {
        Err(r) => r,
        Ok(l) if l.speaking >= sl && l.listening >= sl && l.reading >= rw && l.writing >= rw => {
            req(
                "language",
                Check::Met,
                format!("oral (expression, compréhension) {sl} ou plus, écrit (lecture, écriture) {rw} ou plus"),
                cites,
            )
        }
        Ok(l) => req(
            "language",
            Check::NotMet,
            format!(
                "NCLC expression orale {}, compréhension de l’oral {}, compréhension de l’écrit {}, expression écrite {} (requis : oral {sl}, écrit {rw})",
                l.speaking, l.listening, l.reading, l.writing
            ),
            cites,
        ),
    });
    reqs.push(funds(&p.funds, a));
    reqs.push(quebec(a));
    reqs.push(admissibility());
    ProgramResult {
        program: Program::Fst,
        status: status(&reqs),
        requirements: reqs,
        fsw_points: None,
    }
}

/// Assesses all three programs on `on` (the profile/application date).
pub fn assess(
    grid: &Grid,
    a: &Applicant,
    on: &IsoDate,
) -> Result<Vec<ProgramResult>, EligibilityError> {
    a.validate().map_err(EligibilityError::Invalid)?;
    let p = grid
        .programs
        .as_ref()
        .ok_or_else(|| EligibilityError::NoProgramRules(grid.id.clone()))?;
    Ok(vec![
        fsw(grid, p, a, on),
        cec(grid, p, a, on),
        fst(grid, p, a, on),
    ])
}
