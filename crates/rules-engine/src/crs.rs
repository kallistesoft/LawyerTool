//! CRS score. Mirrors the structure of the official calculator's result
//! (core / spouse / transferability / additional, plus the labour market
//! factor from 2026-11-03), with a citation for every point awarded.

use applicant::{
    Applicant, CanadianEducation, IsoDate, LanguageTest, OfficialLanguage, Skill, TestKind,
    ValidationError,
};
use serde::Serialize;

use crate::grid::{
    Cite, EduClass, Grid, GridIssue, Range, Row, SingleRow, TierRow, TierTable, WageTier,
};
use crate::language::{Clb, LanguageError, to_clb, valid_on};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CrsError {
    #[error("profil du demandeur invalide : {}", join(.0))]
    Invalid(Vec<ValidationError>),
    #[error("la grille {} comporte des lacunes non résolues : {}", .0, join(.1))]
    GridIssues(String, Vec<GridIssue>),
    #[error("aucun test de première langue officielle : non admissible à Entrée express")]
    NoFirstLanguage,
    #[error("{} : {}", language_name(.0), .1)]
    Language(&'static str, LanguageError),
    #[error("aucune ligne du tableau « {table} » pour la valeur {value}")]
    NoRow { table: &'static str, value: u8 },
    #[error("donnée manquante : {}", missing_fr(.0))]
    Missing(&'static str),
}

/// Joins displayable items with "; ".
pub(crate) fn join<T: std::fmt::Display>(items: &[T]) -> String {
    items
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(" ; ")
}

/// French name of an internal language key ("first language"...).
pub fn language_fr(k: &'static str) -> &'static str {
    language_name(k)
}

fn language_name(k: &str) -> &str {
    match k {
        "first language" => "première langue officielle",
        "second language" => "deuxième langue officielle",
        "spouse language" => "langue de l’époux",
        other => other,
    }
}

fn missing_fr(k: &str) -> &str {
    match k {
        "education row" => "ligne de scolarité dans la grille",
        "spouse" => "renseignements sur l’époux",
        "spouse education" => "scolarité de l’époux",
        "spouse education row" => "ligne de scolarité de l’époux dans la grille",
        "high-wage occupation list" => "liste des professions à salaire élevé",
        other => other,
    }
}

/// Something the lawyer must look at; the score is still computed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum Flag {
    /// A language result is not valid on the assessment date.
    ExpiredLanguageTest { which: &'static str },
    /// A value comes from a ruling on a divergence between official sources.
    /// Always accompanied by a `ManualReview` flag with the same id.
    KnownDivergence { id: String },
    /// The lawyer must review this point before the note can be approved.
    ManualReview { id: String, reason: &'static str },
}

impl Flag {
    /// Whether the review screen must block approval until the lawyer
    /// acknowledges this flag.
    pub fn blocks_approval(&self) -> bool {
        matches!(self, Flag::ManualReview { .. })
    }
}

/// One awarded item, with the grid rows it came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Line {
    pub label: &'static str,
    pub points: u16,
    pub cites: Vec<Cite>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Section {
    pub lines: Vec<Line>,
    /// After caps.
    pub subtotal: u16,
}

impl Section {
    fn push(&mut self, label: &'static str, points: u16, cites: Vec<Cite>) {
        self.lines.push(Line {
            label,
            points,
            cites,
        });
    }

    fn sum(&self) -> u16 {
        self.lines.iter().map(|l| l.points).sum()
    }

    fn close(mut self, cap: Option<u16>) -> Self {
        let sum = self.sum();
        self.subtotal = cap.map_or(sum, |c| sum.min(c));
        self
    }

    pub fn line(&self, label: &str) -> Option<u16> {
        self.lines
            .iter()
            .find(|l| l.label == label)
            .map(|l| l.points)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CrsResult {
    pub grid_id: String,
    pub assessed_on: IsoDate,
    pub with_spouse: bool,
    pub core: Section,
    pub spouse: Section,
    pub transferability: Section,
    pub additional: Section,
    /// `None` before 2026-11-03.
    pub labour_market: Option<Section>,
    pub total: u16,
    pub flags: Vec<Flag>,
}

fn find<'a, T>(
    table: &'static str,
    rows: &'a [T],
    value: u8,
    key: impl Fn(&T) -> Range,
) -> Result<&'a T, CrsError> {
    rows.iter()
        .find(|r| key(r).contains(value))
        .ok_or(CrsError::NoRow { table, value })
}

fn row<'a, K>(
    table: &'static str,
    rows: &'a [Row<K>],
    value: u8,
    key: impl Fn(&K) -> Range,
) -> Result<&'a Row<K>, CrsError> {
    find(table, rows, value, |r| key(&r.key))
}

fn single<'a>(
    table: &'static str,
    rows: &'a [SingleRow<Range>],
    value: u8,
) -> Result<&'a SingleRow<Range>, CrsError> {
    find(table, rows, value, |r| r.key)
}

/// Points of a transferability tier table: upper if `level >= upper_min`,
/// lower if `level >= lower_min`, else 0.
fn tier<K>(t: &TierTable<K>, level: u8, row: Option<&TierRow<K>>) -> (u16, Vec<Cite>) {
    let Some(r) = row else { return (0, vec![]) };
    let points = if level >= t.upper_min {
        r.upper
    } else if level >= t.lower_min {
        r.lower
    } else {
        0
    };
    (points, vec![t.cite.clone(), r.cite.clone()])
}

fn edu_row(t: &TierTable<EduClass>, k: EduClass) -> Option<&TierRow<EduClass>> {
    t.rows.iter().find(|r| r.key == k)
}

fn foreign_row(t: &TierTable<Range>, years: u8) -> Option<&TierRow<Range>> {
    t.rows.iter().find(|r| r.key.contains(years))
}

/// Why the lawyer must review a value that rests on a ruling
/// (`fixtures/crs/divergences.md`).
fn review_reason(id: &str) -> &'static str {
    match id {
        "D-001" => {
            "TCF Canada au NCLC 4 (première langue) : 6 points par compétence accordés selon la \
                    page des critères ; la calculatrice officielle donne 0. À confirmer avant de vous fier à cette note."
        }
        "D-002" => {
            "Deuxième langue officielle au NCLC 4 : 0 point selon la décision rendue ; la page des \
                    critères n’a pas de ligne pour le NCLC 4. À confirmer avant de vous fier à cette note."
        }
        "D-003" => {
            "Résultat TEF Canada converti avec le tableau en vigueur à la date du test (page d’IRCC \
                    sur l’immigration au Canada atlantique) ; les pages d’Entrée express ne donnent que \
                    l’échelle d’avant 2019. Confirmez les niveaux NCLC avant de vous fier à cette note."
        }
        "D-004" => {
            "Bonus francophone : 50 points accordés selon la page des critères (français NCLC 7+, \
                    anglais PTE Core NCLC 5+) ; la calculatrice officielle affiche 25 pour PTE Core. \
                    À confirmer avant de vous fier à cette note."
        }
        _ => {
            "Cette valeur repose sur une décision prise sur une divergence entre sources officielles \
              (voir divergences.md). À confirmer avant de vous fier à cette note."
        }
    }
}

/// Every divergence a result relies on is flagged and needs lawyer review.
fn divergence(flags: &mut Vec<Flag>, id: &str) {
    let known = Flag::KnownDivergence { id: id.to_owned() };
    if !flags.contains(&known) {
        flags.push(known);
        flags.push(Flag::ManualReview {
            id: id.to_owned(),
            reason: review_reason(id),
        });
    }
}

/// Converts a result; flags D-003 when a TEF result used a table other than
/// the one on the Express Entry pages (before 2019-09-30).
fn clb(
    grid: &Grid,
    which: &'static str,
    t: &LanguageTest,
    flags: &mut Vec<Flag>,
) -> Result<Clb, CrsError> {
    let (c, table) = to_clb(grid, t).map_err(|e| CrsError::Language(which, e))?;
    if t.test == TestKind::TefCanada && table.taken_from.is_some() {
        divergence(flags, "D-003");
    }
    Ok(c)
}

/// CLB/NCLC levels the scoring core works from: converted from test
/// results ([`score`]) or assumed from an intake form ([`estimate`]).
struct Levels {
    first: (OfficialLanguage, Clb),
    second: Option<(OfficialLanguage, Clb)>,
    /// Spouse's first official language (counted only when scored with spouse).
    spouse: Option<Clb>,
    /// The second-language result is a PTE Core test (ruling D-004).
    pte_second: bool,
}

/// Language levels assumed from an intake form's self-declared levels, the
/// same level in all four skills; `None` = no usable level.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, serde::Deserialize)]
pub struct AssumedLevels {
    pub french: Option<u8>,
    pub english: Option<u8>,
    pub spouse_english: Option<u8>,
}

pub(crate) fn uniform(level: u8) -> Clb {
    Clb {
        speaking: level,
        listening: level,
        reading: level,
        writing: level,
    }
}

/// One official language and its levels.
pub(crate) type LangLevels = (OfficialLanguage, Clb);

/// First and second official language from assumed levels: the higher
/// level first, French on a tie. `None` without any usable level.
pub(crate) fn assumed_order(assumed: &AssumedLevels) -> Option<(LangLevels, Option<LangLevels>)> {
    let fr = assumed
        .french
        .map(|l| (OfficialLanguage::French, uniform(l)));
    let en = assumed
        .english
        .map(|l| (OfficialLanguage::English, uniform(l)));
    match (fr, en) {
        (Some(f), Some(e)) if e.1.min() > f.1.min() => Some((e, Some(f))),
        (Some(f), e) => Some((f, e)),
        (None, Some(e)) => Some((e, None)),
        (None, None) => None,
    }
}

fn check(grid: &Grid, a: &Applicant) -> Result<(), CrsError> {
    a.validate().map_err(CrsError::Invalid)?;
    let issues = grid.issues();
    if !issues.is_empty() {
        return Err(CrsError::GridIssues(grid.id.clone(), issues));
    }
    Ok(())
}

/// Scores `a` against `grid` (already selected for `assessed_on` and with
/// rulings applied).
pub fn score(grid: &Grid, a: &Applicant, assessed_on: &IsoDate) -> Result<CrsResult, CrsError> {
    check(grid, a)?;
    let mut flags = Vec::new();

    let fol_test = a.first_language.as_ref().ok_or(CrsError::NoFirstLanguage)?;
    let fol = clb(grid, "first language", fol_test, &mut flags)?;
    let sol = a
        .second_language
        .as_ref()
        .map(|t| clb(grid, "second language", t, &mut flags))
        .transpose()?;
    for (which, t) in [
        ("first language", Some(fol_test)),
        ("second language", a.second_language.as_ref()),
    ] {
        if t.is_some_and(|t| !valid_on(t, assessed_on)) {
            flags.push(Flag::ExpiredLanguageTest { which });
        }
    }
    // D-001: the calculator gives 0 for TCF at NCLC 4; we follow the page.
    if fol_test.test == TestKind::TcfCanada && Skill::ALL.iter().any(|s| fol.get(*s) == 4) {
        divergence(&mut flags, "D-001");
    }
    let spouse = match a.spouse.as_ref().and_then(|s| s.language.as_ref()) {
        Some(t) if a.scored_with_spouse() => {
            if !valid_on(t, assessed_on) {
                flags.push(Flag::ExpiredLanguageTest {
                    which: "spouse language",
                });
            }
            Some(clb(grid, "spouse language", t, &mut flags)?)
        }
        _ => None,
    };
    let levels = Levels {
        first: (fol_test.test.language(), fol),
        second: a
            .second_language
            .as_ref()
            .zip(sol)
            .map(|(t, c)| (t.test.language(), c)),
        spouse,
        pte_second: a
            .second_language
            .as_ref()
            .is_some_and(|t| t.test == TestKind::PteCore),
    };
    core(grid, a, &levels, assessed_on, flags)
}

/// Scores `a` with language levels assumed from an intake form instead of
/// test results (the profile's own tests are ignored). For a preliminary
/// estimate only: the official score needs real results. The first official
/// language is the one with the higher level (French on a tie).
pub fn estimate(
    grid: &Grid,
    a: &Applicant,
    assumed: &AssumedLevels,
    assessed_on: &IsoDate,
) -> Result<CrsResult, CrsError> {
    check(grid, a)?;
    let (first, second) = assumed_order(assumed).ok_or(CrsError::NoFirstLanguage)?;
    let levels = Levels {
        first,
        second,
        spouse: assumed
            .spouse_english
            .filter(|_| a.scored_with_spouse())
            .map(uniform),
        pte_second: false,
    };
    core(grid, a, &levels, assessed_on, Vec::new())
}

fn core(
    grid: &Grid,
    a: &Applicant,
    levels: &Levels,
    assessed_on: &IsoDate,
    mut flags: Vec<Flag>,
) -> Result<CrsResult, CrsError> {
    let c = &grid.crs;
    let ws = a.scored_with_spouse();
    let fol = levels.first.1;
    let sol = levels.second.map(|(_, c)| c);

    // A. Core / human capital.
    let mut core = Section::default();
    let r = row("age", &c.age, a.age, |k| *k)?;
    core.push("age", r.points.get(ws), vec![r.cite.clone()]);
    let r = c
        .education
        .iter()
        .find(|r| r.key == a.education)
        .ok_or(CrsError::Missing("education row"))?;
    core.push("education", r.points.get(ws), vec![r.cite.clone()]);

    let per_skill =
        |rows: &[Row<Range>], table, levels: &Clb| -> Result<(u16, Vec<Cite>), CrsError> {
            let mut points = 0;
            let mut cites = Vec::new();
            for s in Skill::ALL {
                let r = row(table, rows, levels.get(s), |k| *k)?;
                points += r.points.get(ws);
                cites.push(r.cite.clone());
            }
            Ok((points, cites))
        };
    let (p, cites) = per_skill(&c.first_language, "first_language", &fol)?;
    core.push("first official language", p, cites);
    if let Some(sol) = &sol {
        let (p, mut cites) = per_skill(&c.second_language, "second_language", sol)?;
        let cap = c.second_language_max.points.get(ws);
        cites.push(c.second_language_max.cite.clone());
        core.push("second official language", p.min(cap), cites);
    }
    if c.labour_market.is_none() {
        let r = row(
            "canadian_work",
            &c.canadian_work,
            a.canadian_work_years,
            |k| *k,
        )?;
        core.push(
            "Canadian work experience",
            r.points.get(ws),
            vec![r.cite.clone()],
        );
    }
    let core = core.close(None);

    // B. Spouse.
    let mut spouse = Section::default();
    if ws {
        let s = a.spouse.as_ref().ok_or(CrsError::Missing("spouse"))?;
        let edu = s.education.ok_or(CrsError::Missing("spouse education"))?;
        let r = c
            .spouse_education
            .iter()
            .find(|r| r.key == edu)
            .ok_or(CrsError::Missing("spouse education row"))?;
        spouse.push("spouse education", r.points, vec![r.cite.clone()]);
        if let Some(levels) = levels.spouse {
            let mut points = 0;
            let mut cites = Vec::new();
            for sk in Skill::ALL {
                let r = single("spouse_language", &c.spouse_language, levels.get(sk))?;
                points += r.points;
                cites.push(r.cite.clone());
            }
            spouse.push("spouse first official language", points, cites);
        }
        let r = single(
            "spouse_canadian_work",
            &c.spouse_canadian_work,
            s.canadian_work_years,
        )?;
        spouse.push(
            "spouse Canadian work experience",
            r.points,
            vec![r.cite.clone()],
        );
    }
    let spouse = spouse.close(Some(c.spouse_max.value));

    // C. Skill transferability (first official language only).
    let t = &c.transferability;
    let group = t.group_max.value;
    let edu_class = EduClass::from(a.education);
    let fol_min = fol.min();
    let mut transfer = Section::default();

    let (ea, mut ec) = tier(
        &t.education_language,
        fol_min,
        edu_row(&t.education_language, edu_class),
    );
    let (eb, cb) = tier(
        &t.education_canadian_work,
        a.canadian_work_years,
        edu_row(&t.education_canadian_work, edu_class),
    );
    ec.extend(cb);
    ec.push(t.group_max.cite.clone());
    transfer.push("education", (ea + eb).min(group), ec);

    let (fa, mut fc) = tier(
        &t.foreign_language,
        fol_min,
        foreign_row(&t.foreign_language, a.foreign_work_years),
    );
    let (fb, cb) = tier(
        &t.foreign_canadian_work,
        a.canadian_work_years,
        foreign_row(&t.foreign_canadian_work, a.foreign_work_years),
    );
    fc.extend(cb);
    fc.push(t.group_max.cite.clone());
    transfer.push("foreign work experience", (fa + fb).min(group), fc);

    if a.certificate_of_qualification {
        let (p, cites) = tier(&t.certificate, fol_min, t.certificate.rows.first());
        transfer.push("certificate of qualification", p.min(group), cites);
    }
    let transfer = transfer.close(Some(t.max.value));

    // D. Additional points.
    let x = &c.additional;
    let mut additional = Section::default();
    if a.provincial_nomination {
        additional.push(
            "provincial nomination",
            x.provincial_nomination.points,
            vec![x.provincial_nomination.cite.clone()],
        );
    }
    match a.canadian_education {
        Some(CanadianEducation::OneOrTwoYear) => additional.push(
            "Canadian education",
            x.canadian_education_short.points,
            vec![x.canadian_education_short.cite.clone()],
        ),
        Some(CanadianEducation::ThreeYearOrMore) => additional.push(
            "Canadian education",
            x.canadian_education_long.points,
            vec![x.canadian_education_long.cite.clone()],
        ),
        Some(CanadianEducation::SecondaryOrLess) | None => {}
    }
    if a.sibling_in_canada {
        additional.push(
            "sibling in Canada",
            x.sibling.points,
            vec![x.sibling.cite.clone()],
        );
    }
    let (mut french, mut english) = (None, None);
    for (lang, l) in [Some(levels.first), levels.second].into_iter().flatten() {
        match lang {
            OfficialLanguage::French => french = Some(l),
            OfficialLanguage::English => english = Some(l),
        }
    }
    if french.is_some_and(|f| f.all_at_least(x.french_min_nclc)) {
        let strong = english.is_some_and(|e| e.all_at_least(x.english_min_clb));
        let r = if strong {
            &x.french_strong_english
        } else {
            &x.french_weak_english
        };
        additional.push("French language skills", r.points, vec![r.cite.clone()]);
        // D-004: the calculator gives 25 here when the English test is PTE Core.
        if strong && levels.first.0 == OfficialLanguage::French && levels.pte_second {
            divergence(&mut flags, "D-004");
        }
    }
    let additional = additional.close(Some(x.max.value));

    // Labour market integration (2026-11-03 onward).
    let labour_market = c
        .labour_market
        .as_ref()
        .map(|lm| -> Result<Section, CrsError> {
            let mut s = Section::default();
            let dur = row(
                "labour_market.duration",
                &lm.duration,
                a.canadian_work_years,
                |k| *k,
            )?;
            let mut cw_points = dur.points.get(ws);
            let mut cw_cites = vec![dur.cite.clone()];

            if a.canadian_work_years >= 1 {
                if a.canadian_work_by_noc.is_empty() {
                    return Err(CrsError::Missing(
                        "Canadian work by occupation (NOC and months)",
                    ));
                }
                let list = c
                    .high_wage
                    .as_ref()
                    .ok_or(CrsError::Missing("high-wage occupation list"))?;
                let months_at = |min: WageTier| -> u16 {
                    a.canadian_work_by_noc
                        .iter()
                        .filter(|w| list.tier(w.noc.as_str()).is_some_and(|t| t >= min))
                        .map(|w| w.months)
                        .sum()
                };
                let best = lm
                    .high_wage_experience
                    .iter()
                    .filter(|r| months_at(r.key) >= lm.high_wage_min_months)
                    .max_by_key(|r| r.points);
                if let Some(r) = best {
                    cw_points += r.points;
                    cw_cites.push(r.cite.clone());
                    cw_cites.extend(
                        a.canadian_work_by_noc
                            .iter()
                            .filter_map(|w| {
                                list.occupations
                                    .iter()
                                    .find(|o| o.noc == w.noc.as_str() && o.tier >= r.key)
                            })
                            .map(|o| o.cite.clone()),
                    );
                }
            }
            let offer = a.job_offer.as_ref().and_then(|o| {
                let list = c.high_wage.as_ref()?;
                let tier = list.tier(o.noc.as_str())?;
                let r = lm.job_offer.iter().find(|r| r.key == tier)?;
                let occ = list.occupations.iter().find(|x| x.noc == o.noc.as_str())?;
                Some((r.points, vec![r.cite.clone(), occ.cite.clone()]))
            });

            // The greater of the two options counts, never both.
            match offer {
                Some((p, cites)) if p > cw_points => s.push("qualifying job offer", p, cites),
                _ => s.push("Canadian work experience", cw_points, cw_cites),
            }
            let mut s = s.close(Some(lm.max.points.get(ws)));
            if let Some(l) = s.lines.first_mut() {
                l.cites.push(lm.max.cite.clone());
            }
            Ok(s)
        });
    let labour_market = labour_market.transpose()?;

    let total = core.subtotal
        + spouse.subtotal
        + transfer.subtotal
        + additional.subtotal
        + labour_market.as_ref().map_or(0, |s| s.subtotal);

    for s in [&core, &spouse, &transfer, &additional]
        .into_iter()
        .chain(labour_market.as_ref())
    {
        for l in &s.lines {
            for cite in &l.cites {
                if let Some(id) = cite.source.strip_prefix("ruling:") {
                    divergence(&mut flags, id);
                }
            }
        }
    }

    Ok(CrsResult {
        grid_id: grid.id.clone(),
        assessed_on: assessed_on.clone(),
        with_spouse: ws,
        core,
        spouse,
        transferability: transfer,
        additional,
        labour_market,
        total,
        flags,
    })
}
