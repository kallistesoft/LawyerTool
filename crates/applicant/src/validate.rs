use crate::{Applicant, LanguageTest, Skill};

/// A reason an applicant record is impossible or incomplete. The engine
/// refuses to score invalid applicants rather than guessing.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ValidationError {
    #[error("spouse details given but marital status has no partner")]
    SpouseWithoutPartner,
    #[error("married/common-law applicant is missing spouse details")]
    MissingSpouse,
    #[error("spouse is scored but spouse education is missing")]
    MissingSpouseEducation,
    #[error("age {0} is out of range")]
    Age(u8),
    #[error("{field}: {years} years of work is impossible at age {age}")]
    WorkYears {
        field: &'static str,
        years: u8,
        age: u8,
    },
    #[error("{who} {skill:?} score {score} is invalid for {test:?}")]
    LanguageScore {
        who: &'static str,
        test: crate::TestKind,
        skill: Skill,
        score: u16,
    },
    #[error("second language test must be in the other official language")]
    SecondLanguageSameAsFirst,
    #[error("second language test given without a first language test")]
    SecondWithoutFirst,
    #[error("family size must be at least 1")]
    FamilySize,
    #[error("too many {what} (at most {max})")]
    TooMany { what: &'static str, max: usize },
    #[error("work period {index}: {reason}")]
    WorkPeriod { index: usize, reason: &'static str },
    #[error(
        "Canadian work by occupation totals {months} months but canadian_work_years is {years}"
    )]
    CanadianWorkMismatch { months: u16, years: u8 },
}

/// List limits: far above any real file, bounded for hostile imports.
pub const MAX_WORK_PERIODS: usize = 100;
pub const MAX_NOC_ROWS: usize = 50;

/// Youngest age at which skilled work is plausibly counted.
const MIN_WORK_AGE: u8 = 15;

impl Applicant {
    /// Returns every problem found, not just the first.
    pub fn validate(&self) -> Result<(), Vec<ValidationError>> {
        let mut errs = Vec::new();

        if !(1..=120).contains(&self.age) {
            errs.push(ValidationError::Age(self.age));
        }

        match (self.marital_status.has_partner(), &self.spouse) {
            (false, Some(_)) => errs.push(ValidationError::SpouseWithoutPartner),
            (true, None) => errs.push(ValidationError::MissingSpouse),
            _ => {}
        }

        if let Some(s) = &self.spouse {
            if self.scored_with_spouse() && s.education.is_none() {
                errs.push(ValidationError::MissingSpouseEducation);
            }
            if let Some(t) = &s.language {
                check_test("spouse", t, &mut errs);
            }
        }

        let max_work = self.age.saturating_sub(MIN_WORK_AGE);
        for (field, years) in [
            ("canadian_work_years", self.canadian_work_years),
            ("foreign_work_years", self.foreign_work_years),
        ] {
            if years > max_work {
                errs.push(ValidationError::WorkYears {
                    field,
                    years,
                    age: self.age,
                });
            }
        }

        if let Some(t) = &self.first_language {
            check_test("first language", t, &mut errs);
        }
        if let Some(t) = &self.second_language {
            check_test("second language", t, &mut errs);
            match &self.first_language {
                None => errs.push(ValidationError::SecondWithoutFirst),
                Some(f) if f.test.language() == t.test.language() => {
                    errs.push(ValidationError::SecondLanguageSameAsFirst)
                }
                Some(_) => {}
            }
        }

        if !self.canadian_work_by_noc.is_empty() {
            let months: u16 = self.canadian_work_by_noc.iter().map(|w| w.months).sum();
            if months / crate::MONTHS_PER_YEAR != u16::from(self.canadian_work_years) {
                errs.push(ValidationError::CanadianWorkMismatch {
                    months,
                    years: self.canadian_work_years,
                });
            }
        }

        for (what, n, max) in [
            (
                "work periods",
                self.eligibility.work_history.len(),
                MAX_WORK_PERIODS,
            ),
            ("NOC rows", self.canadian_work_by_noc.len(), MAX_NOC_ROWS),
        ] {
            if n > max {
                errs.push(ValidationError::TooMany { what, max });
            }
        }
        for (i, w) in self.eligibility.work_history.iter().enumerate() {
            if w.end < w.start {
                errs.push(ValidationError::WorkPeriod {
                    index: i,
                    reason: "ends before it starts",
                });
            }
            if w.hours_per_week == 0 || w.hours_per_week > 120 {
                errs.push(ValidationError::WorkPeriod {
                    index: i,
                    reason: "hours per week out of range",
                });
            }
        }

        if self.eligibility.family_size == Some(0) {
            errs.push(ValidationError::FamilySize);
        }

        if errs.is_empty() { Ok(()) } else { Err(errs) }
    }
}

fn check_test(who: &'static str, t: &LanguageTest, errs: &mut Vec<ValidationError>) {
    for skill in Skill::ALL {
        let score = t.score(skill);
        let (lo, hi, step) = t.test.score_range(skill);
        if score < lo || score > hi || !(score - lo).is_multiple_of(step) {
            errs.push(ValidationError::LanguageScore {
                who,
                test: t.test,
                skill,
                score,
            });
        }
    }
}
