//! Canonical applicant model.
//!
//! Field set mirrors the IRCC "Calculatrice du SCG" questions (q1..q12, see
//! `crates/rules-engine/fixtures/sources/verifier-note.html`) plus the extra
//! facts needed for program eligibility (FSW / CEC / FST).
//!
//! Language results are stored as raw test scores, never as CLB/NCLC levels:
//! conversion belongs to the rules engine, driven by an approved grid snapshot.

mod language;
mod validate;

pub use language::{LanguageTest, OfficialLanguage, Skill, TestKind};
pub use validate::ValidationError;

use serde::{Deserialize, Serialize};

/// Calculator q1. Only `Married` and `CommonLaw` can carry a spouse.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MaritalStatus {
    Annulled,
    CommonLaw,
    Divorced,
    LegallySeparated,
    Married,
    NeverMarried,
    Widowed,
}

impl MaritalStatus {
    pub fn has_partner(self) -> bool {
        matches!(self, Self::Married | Self::CommonLaw)
    }
}

/// Highest credential (calculator q4 / spouse q10, codes A..H).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EducationLevel {
    LessThanSecondary,
    Secondary,
    OneYearPostSecondary,
    TwoYearPostSecondary,
    /// Bachelor's or other program of three or more years.
    BachelorOrThreeYear,
    /// Two or more credentials, at least one of three years or more.
    TwoOrMoreCredentials,
    /// Master's or professional degree required for a licensed profession.
    MasterOrProfessional,
    Doctorate,
}

/// Canadian post-secondary credential (calculator q4b/q4c).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CanadianEducation {
    SecondaryOrLess,
    OneOrTwoYear,
    /// Three years or more, or a master's / professional / doctoral degree of
    /// at least one academic year.
    ThreeYearOrMore,
}

/// NOC category of an arranged-employment offer (calculator q8a).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobOfferNoc {
    /// NOC TEER 0, major group 00.
    Teer0Major00,
    /// TEER 1, 2 or 3, or TEER 0 outside major group 00.
    Teer0123,
    Teer45,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Spouse {
    /// Calculator q2i. A citizen/PR spouse is scored as "without spouse".
    pub is_canadian_citizen_or_pr: bool,
    /// Calculator q2ii. A non-accompanying spouse is scored as "without spouse".
    pub accompanying: bool,
    /// Calculator q10. Required when the spouse counts in the score.
    pub education: Option<EducationLevel>,
    /// Calculator q11, whole years.
    pub canadian_work_years: u8,
    /// Calculator q12. `None` = "sans objet" (no test).
    pub language: Option<LanguageTest>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Applicant {
    pub marital_status: MaritalStatus,
    /// Present iff `marital_status.has_partner()`.
    pub spouse: Option<Spouse>,
    /// Age in whole years on the assessment date (calculator q3).
    pub age: u8,
    pub education: EducationLevel,
    /// `None` = no Canadian credential (calculator q4b = "Non").
    pub canadian_education: Option<CanadianEducation>,
    /// Calculator q5i. `None` = no approved test, which makes the applicant
    /// ineligible for Express Entry.
    pub first_language: Option<LanguageTest>,
    /// Calculator q5ii. Must be in the other official language.
    pub second_language: Option<LanguageTest>,
    /// Skilled Canadian work in the last 10 years, whole years (q6i).
    pub canadian_work_years: u8,
    /// Skilled foreign work in the last 10 years, whole years (q6ii).
    pub foreign_work_years: u8,
    /// Certificate of qualification from a province/territory (q7).
    pub certificate_of_qualification: bool,
    /// Valid arranged-employment offer (q8/q8a).
    pub job_offer: Option<JobOfferNoc>,
    /// Provincial/territorial nomination (q9).
    pub provincial_nomination: bool,
    /// Sibling 18+ in Canada who is a citizen or PR (q10i).
    pub sibling_in_canada: bool,

    // Eligibility-only facts (not asked by the CRS calculator).
    /// Settlement funds available, whole CAD. `None` = unknown.
    #[serde(default)]
    pub settlement_funds_cad: Option<u32>,
    /// Family members counted for the funds requirement, applicant included.
    #[serde(default)]
    pub family_size: Option<u8>,
}

impl Applicant {
    /// Whether CRS uses the "with spouse" columns (calculator `with_spouse`).
    pub fn scored_with_spouse(&self) -> bool {
        self.marital_status.has_partner()
            && self
                .spouse
                .as_ref()
                .is_some_and(|s| !s.is_canadian_citizen_or_pr && s.accompanying)
    }
}
