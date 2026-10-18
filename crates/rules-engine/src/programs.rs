//! Express Entry program rules (FSW, CEC, FST) and proof of funds, as data.
//! Parsed from the official program pages; every value is cited.

use applicant::EducationLevel;
use serde::{Deserialize, Serialize};

use crate::grid::{Cap, Cite, Range, SingleRow};

/// A NOC prefix that is in the trades list, minus excluded sub-prefixes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NocGroup {
    pub prefix: String,
    pub except: Vec<String>,
}

impl NocGroup {
    pub fn contains(&self, noc: &str) -> bool {
        noc.starts_with(&self.prefix) && !self.except.iter().any(|e| noc.starts_with(e.as_str()))
    }
}

/// Work-experience minimum: `min_hours` within `window_years` before the
/// application, counting at most `max_hours_per_week`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExperienceRule {
    pub min_hours: Cap,
    pub window_years: Cap,
    pub max_hours_per_week: Cap,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Adaptability {
    /// Spouse has CLB/NCLC `spouse_language_min` or more on all four skills.
    pub spouse_language: SingleRow<()>,
    pub spouse_language_min: u8,
    pub own_canadian_study: SingleRow<()>,
    pub spouse_canadian_study: SingleRow<()>,
    pub own_canadian_work: SingleRow<()>,
    pub spouse_canadian_work: SingleRow<()>,
    pub arranged_employment: SingleRow<()>,
    pub relative_in_canada: SingleRow<()>,
    pub max: Cap,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fsw {
    pub experience: ExperienceRule,
    /// Minimum CLB/NCLC on all four skills, first official language.
    pub min_clb: Cap,
    /// Points per skill, keyed by CLB (no row below the minimum).
    pub first_language: Vec<SingleRow<Range>>,
    /// Points if the second language is at `second_language_min` or more on
    /// all four skills.
    pub second_language: SingleRow<()>,
    pub second_language_min: u8,
    pub education: Vec<SingleRow<EducationLevel>>,
    /// Keyed by years of skilled work experience.
    pub work_experience: Vec<SingleRow<Range>>,
    pub age: Vec<SingleRow<Range>>,
    pub arranged_employment: Cap,
    pub adaptability: Adaptability,
    pub pass_mark: Cap,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cec {
    pub experience: ExperienceRule,
    /// Minimum CLB/NCLC, all four skills, for experience in TEER 0 or 1.
    pub min_clb_teer_0_1: Cap,
    /// For experience in TEER 2 or 3.
    pub min_clb_teer_2_3: Cap,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fst {
    pub experience: ExperienceRule,
    pub trades: Vec<NocGroup>,
    pub trades_cite: Cite,
    pub min_clb_speaking_listening: Cap,
    pub min_clb_reading_writing: Cap,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FundsRow {
    pub family_size: u8,
    pub amount_cad: u32,
    pub cite: Cite,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Funds {
    pub rows: Vec<FundsRow>,
    /// Added per family member beyond the last row.
    pub per_extra_member_cad: u32,
    pub extra_cite: Cite,
    /// "Mise à jour" date printed in the table header.
    pub updated: String,
}

impl Funds {
    pub fn required(&self, family_size: u8) -> Option<(u32, Vec<Cite>)> {
        if let Some(r) = self.rows.iter().find(|r| r.family_size == family_size) {
            return Some((r.amount_cad, vec![r.cite.clone()]));
        }
        let last = self.rows.iter().max_by_key(|r| r.family_size)?;
        let extra = u32::from(family_size.checked_sub(last.family_size)?);
        Some((
            last.amount_cad + extra * self.per_extra_member_cad,
            vec![last.cite.clone(), self.extra_cite.clone()],
        ))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Programs {
    pub fsw: Fsw,
    pub cec: Cec,
    pub fst: Fst,
    pub funds: Funds,
}
