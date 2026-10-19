//! Language test results → CLB/NCLC levels, using the grid's equivalency
//! tables (ruling D-003: TEF Canada table chosen by test date).

use applicant::{IsoDate, LanguageTest, Skill, TestKind};
use serde::Serialize;

use crate::grid::{EquivalencyTable, Grid};

/// Express Entry results must be less than two years old
/// (`examen-linguistique.html`, "Durée de validité de vos résultats").
pub const VALIDITY_YEARS: u16 = 2;

/// CLB (English) or NCLC (French) level per skill. A value below the lowest
/// row of the table (CLB 4, or 3 for PTE Core) means "below that level".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Clb {
    pub speaking: u8,
    pub listening: u8,
    pub reading: u8,
    pub writing: u8,
}

impl Clb {
    pub fn get(&self, skill: Skill) -> u8 {
        match skill {
            Skill::Speaking => self.speaking,
            Skill::Listening => self.listening,
            Skill::Reading => self.reading,
            Skill::Writing => self.writing,
        }
    }

    pub fn min(&self) -> u8 {
        Skill::ALL.iter().map(|s| self.get(*s)).min().unwrap_or(0)
    }

    pub fn all_at_least(&self, level: u8) -> bool {
        self.min() >= level
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LanguageError {
    #[error("aucun tableau d’équivalence {test} pour un test passé le {date}")]
    NoTable { test: TestKind, date: String },
    #[error("plusieurs tableaux d’équivalence {test} pour un test passé le {date}")]
    AmbiguousTable { test: TestKind, date: String },
    #[error(
        "{test} : le résultat {score} en {skill} dépasse l’échelle en vigueur à la date du test"
    )]
    OutOfScale {
        test: TestKind,
        skill: Skill,
        score: u16,
    },
}

/// The single table that applies to this test and date.
pub fn table<'a>(
    grid: &'a Grid,
    test: TestKind,
    date: &IsoDate,
) -> Result<&'a EquivalencyTable, LanguageError> {
    let found: Vec<_> = grid
        .language
        .iter()
        .filter(|t| t.test == test && t.applies_to(date))
        .collect();
    let date = date.as_str().to_owned();
    match found[..] {
        [t] => Ok(t),
        [] => Err(LanguageError::NoTable { test, date }),
        _ => Err(LanguageError::AmbiguousTable { test, date }),
    }
}

fn level(t: &EquivalencyTable, skill: Skill, score: u16) -> Result<u8, LanguageError> {
    let lowest = t.rows.first().map_or(0, |r| r.clb);
    match t.rows.iter().rev().find(|r| r.band(skill).min <= score) {
        None => Ok(lowest.saturating_sub(1)),
        Some(r) if r.band(skill).max.is_some_and(|m| score > m) => Err(LanguageError::OutOfScale {
            test: t.test,
            skill,
            score,
        }),
        Some(r) => Ok(r.clb),
    }
}

/// Converts one test result. Returns the levels and the table used (for
/// citation in notes).
pub fn to_clb<'a>(
    grid: &'a Grid,
    result: &LanguageTest,
) -> Result<(Clb, &'a EquivalencyTable), LanguageError> {
    let t = table(grid, result.test, &result.test_date)?;
    let l = |s: Skill| level(t, s, result.score(s));
    let clb = Clb {
        speaking: l(Skill::Speaking)?,
        listening: l(Skill::Listening)?,
        reading: l(Skill::Reading)?,
        writing: l(Skill::Writing)?,
    };
    Ok((clb, t))
}

/// Whether the result is still valid on `date` (taken on or before it, and
/// no more than two years old).
pub fn valid_on(result: &LanguageTest, date: &IsoDate) -> bool {
    result.test_date <= *date && *date <= result.test_date.plus_years(VALIDITY_YEARS)
}
