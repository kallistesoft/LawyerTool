//! Rules grid: the data the engine scores against.
//!
//! A grid is produced by `grid-parser` from official pages, approved by the
//! lawyer, and stored as a versioned snapshot. Every value carries a [`Cite`]
//! pointing back to the exact table row it came from, so notes can quote it.

use applicant::{EducationLevel, Skill, TestKind};
use serde::{Deserialize, Serialize};

pub use applicant::IsoDate;

/// Where a value came from: source id, table and row index in that page's
/// `<main>` (0-based, header row included; `None` for running text), and the
/// verbatim text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cite {
    pub source: String,
    pub table: Option<u16>,
    pub row: Option<u16>,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Source {
    pub id: String,
    pub url: String,
    pub sha256: String,
    pub fetched: IsoDate,
}

/// Inclusive integer range; `max: None` means "or more".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Range {
    pub min: u8,
    pub max: Option<u8>,
}

impl Range {
    pub fn contains(&self, v: u8) -> bool {
        v >= self.min && self.max.is_none_or(|m| v <= m)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Points {
    pub with_spouse: u16,
    pub without_spouse: u16,
}

impl Points {
    pub fn get(&self, with_spouse: bool) -> u16 {
        if with_spouse {
            self.with_spouse
        } else {
            self.without_spouse
        }
    }
}

/// Row with separate with/without-spouse columns.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Row<K> {
    pub key: K,
    pub points: Points,
    pub cite: Cite,
}

/// Row with a single points column.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SingleRow<K> {
    pub key: K,
    pub points: u16,
    pub cite: Cite,
}

/// Skill-transferability row: points at the lower and the upper tier.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TierRow<K> {
    pub key: K,
    pub lower: u16,
    pub upper: u16,
    pub cite: Cite,
}

/// Education classes used by the skill-transferability tables.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EduClass {
    SecondaryOrLess,
    /// Post-secondary credential of one year or longer.
    PostSecondary,
    TwoOrMore,
    MasterOrProfessional,
    Doctorate,
}

impl From<EducationLevel> for EduClass {
    fn from(e: EducationLevel) -> Self {
        use EducationLevel::*;
        match e {
            LessThanSecondary | Secondary => Self::SecondaryOrLess,
            OneYearPostSecondary | TwoYearPostSecondary | BachelorOrThreeYear => {
                Self::PostSecondary
            }
            TwoOrMoreCredentials => Self::TwoOrMore,
            MasterOrProfessional => Self::MasterOrProfessional,
            Doctorate => Self::Doctorate,
        }
    }
}

/// A cap value with its citation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cap {
    pub value: u16,
    pub cite: Cite,
}

/// Transferability table with its two tier thresholds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TierTable<K> {
    /// Minimum level (CLB, or years of Canadian work) for the lower tier.
    pub lower_min: u8,
    /// Minimum level for the upper tier.
    pub upper_min: u8,
    pub rows: Vec<TierRow<K>>,
    /// Header row, quoted.
    pub cite: Cite,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Transferability {
    /// Education + first-language CLB on all four skills.
    pub education_language: TierTable<EduClass>,
    /// Education + years of Canadian work.
    pub education_canadian_work: TierTable<EduClass>,
    /// Foreign work years + first-language CLB.
    pub foreign_language: TierTable<Range>,
    /// Foreign work years + years of Canadian work.
    pub foreign_canadian_work: TierTable<Range>,
    /// Certificate of qualification + first-language CLB.
    pub certificate: TierTable<()>,
    /// Cap for each of the education / foreign-work / certificate groups.
    pub group_max: Cap,
    pub max: Cap,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Additional {
    pub sibling: SingleRow<()>,
    /// NCLC 7+ in French on all skills, English CLB 4 or less (or no test).
    pub french_weak_english: SingleRow<()>,
    /// NCLC 7+ in French on all skills, English CLB 5+ on all skills.
    pub french_strong_english: SingleRow<()>,
    pub french_min_nclc: u8,
    pub english_min_clb: u8,
    pub canadian_education_short: SingleRow<()>,
    pub canadian_education_long: SingleRow<()>,
    pub provincial_nomination: SingleRow<()>,
    pub max: Cap,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Crs {
    /// Core / human-capital cap, with and without spouse.
    pub core_max: Row<()>,
    pub age: Vec<Row<Range>>,
    pub education: Vec<Row<EducationLevel>>,
    /// Points per skill, keyed by CLB range.
    pub first_language: Vec<Row<Range>>,
    pub second_language: Vec<Row<Range>>,
    pub second_language_max: Row<()>,
    /// Keyed by years of Canadian work.
    pub canadian_work: Vec<Row<Range>>,
    pub spouse_max: Cap,
    pub spouse_education: Vec<SingleRow<EducationLevel>>,
    /// Points per skill, keyed by CLB range.
    pub spouse_language: Vec<SingleRow<Range>>,
    pub spouse_canadian_work: Vec<SingleRow<Range>>,
    pub transferability: Transferability,
    pub additional: Additional,
    /// Labour market integration factor (from 2026-11-03). When present,
    /// Canadian work points move out of the core factors into this factor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub labour_market: Option<LabourMarket>,
    /// High-wage occupation list used by `labour_market`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub high_wage: Option<HighWageList>,
}

/// High-wage tier of a NOC: national median wage relative to the average
/// median wage of the Canadian workforce.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WageTier {
    /// At least 1.3×, under 1.5×.
    AtLeast1_3,
    /// At least 1.5×, under 2×.
    AtLeast1_5,
    /// At least 2×.
    AtLeast2,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LabourMarket {
    /// Factor cap, with and without spouse.
    pub max: Row<()>,
    /// Duration points, keyed by years of Canadian work.
    pub duration: Vec<Row<Range>>,
    /// Extra points for at least `high_wage_min_months` of Canadian work in
    /// occupations at this tier or higher.
    pub high_wage_experience: Vec<SingleRow<WageTier>>,
    pub high_wage_min_months: u16,
    /// Points for a qualifying job offer in an occupation at this tier.
    pub job_offer: Vec<SingleRow<WageTier>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Occupation {
    /// NOC 2021 code, five digits.
    pub noc: String,
    pub tier: WageTier,
    pub cite: Cite,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HighWageList {
    pub effective_from: IsoDate,
    pub occupations: Vec<Occupation>,
}

impl HighWageList {
    pub fn tier(&self, noc: &str) -> Option<WageTier> {
        self.occupations
            .iter()
            .find(|o| o.noc == noc)
            .map(|o| o.tier)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Grid {
    pub id: String,
    pub effective_from: IsoDate,
    pub sources: Vec<Source>,
    pub crs: Crs,
    /// Test score → CLB/NCLC conversion tables.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub language: Vec<EquivalencyTable>,
    /// FSW / CEC / FST minimum requirements and proof of funds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub programs: Option<crate::programs::Programs>,
}

/// Inclusive raw-score band; `max: None` = open-ended (top row of a table
/// that gives a single threshold, e.g. CELPIP "10").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScoreBand {
    pub min: u16,
    pub max: Option<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EquivalencyRow {
    pub clb: u8,
    pub speaking: ScoreBand,
    pub listening: ScoreBand,
    pub reading: ScoreBand,
    pub writing: ScoreBand,
    pub cite: Cite,
}

impl EquivalencyRow {
    pub fn band(&self, skill: Skill) -> ScoreBand {
        match skill {
            Skill::Speaking => self.speaking,
            Skill::Listening => self.listening,
            Skill::Reading => self.reading,
            Skill::Writing => self.writing,
        }
    }
}

/// One test's conversion table, valid for tests taken in
/// `taken_from..=taken_to` (open ends = unbounded).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EquivalencyTable {
    pub test: TestKind,
    pub taken_from: Option<IsoDate>,
    pub taken_to: Option<IsoDate>,
    /// Ascending by CLB.
    pub rows: Vec<EquivalencyRow>,
    pub cite: Cite,
}

impl EquivalencyTable {
    pub fn applies_to(&self, date: &IsoDate) -> bool {
        self.taken_from.as_ref().is_none_or(|f| f <= date)
            && self.taken_to.as_ref().is_none_or(|t| date <= t)
    }
}

/// Picks the grid in force on `date`: the latest `effective_from` not after it.
pub fn in_force<'a>(grids: &'a [Grid], date: &IsoDate) -> Option<&'a Grid> {
    grids
        .iter()
        .filter(|g| g.effective_from <= *date)
        .max_by(|a, b| a.effective_from.cmp(&b.effective_from))
}

/// A structural problem in a grid: a value range not covered by any row, or
/// covered twice. The engine refuses to score against a grid with issues
/// unless each one is resolved by a ruling.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GridIssue {
    Gap { table: &'static str, value: u8 },
    Overlap { table: &'static str, value: u8 },
}

/// Checks that `ranges` cover 0..=∞ exactly once, assuming rows sorted by `min`.
fn coverage(table: &'static str, ranges: impl Iterator<Item = Range>, out: &mut Vec<GridIssue>) {
    let mut next: Option<u8> = Some(0); // smallest value not yet covered; None = covered to ∞
    for r in ranges {
        match next {
            None => out.push(GridIssue::Overlap {
                table,
                value: r.min,
            }),
            Some(n) if r.min < n => out.push(GridIssue::Overlap {
                table,
                value: r.min,
            }),
            Some(n) => out.extend((n..r.min).map(|value| GridIssue::Gap { table, value })),
        }
        next = r.max.and_then(|m| m.checked_add(1));
        if r.max.is_none() {
            next = None;
        }
    }
    if let Some(n) = next {
        out.push(GridIssue::Gap { table, value: n });
    }
}

impl Grid {
    pub fn issues(&self) -> Vec<GridIssue> {
        let c = &self.crs;
        let t = &c.transferability;
        let mut out = Vec::new();
        coverage("age", c.age.iter().map(|r| r.key), &mut out);
        coverage(
            "first_language",
            c.first_language.iter().map(|r| r.key),
            &mut out,
        );
        coverage(
            "second_language",
            c.second_language.iter().map(|r| r.key),
            &mut out,
        );
        coverage(
            "canadian_work",
            c.canadian_work.iter().map(|r| r.key),
            &mut out,
        );
        coverage(
            "spouse_language",
            c.spouse_language.iter().map(|r| r.key),
            &mut out,
        );
        coverage(
            "spouse_canadian_work",
            c.spouse_canadian_work.iter().map(|r| r.key),
            &mut out,
        );
        coverage(
            "foreign_language",
            t.foreign_language.rows.iter().map(|r| r.key),
            &mut out,
        );
        coverage(
            "foreign_canadian_work",
            t.foreign_canadian_work.rows.iter().map(|r| r.key),
            &mut out,
        );
        out
    }
}
