//! Language test → CLB/NCLC tables (`equivalences-aip-en.html`, ruling D-003).
//!
//! Each table sits in `<details>` blocks whose `<summary>` names the test and,
//! for TEF Canada, the range of test dates the table applies to.

use applicant::{IsoDate, Skill, TestKind};
use rules_engine::grid::{EquivalencyRow, EquivalencyTable, ScoreBand};

use crate::ParseError;
use crate::crs::re;
use crate::page::{Page, Table};

const TESTS: &[(&str, TestKind)] = &[
    ("CELPIP (General)", TestKind::CelpipGeneral),
    ("IELTS (General Training)", TestKind::IeltsGeneral),
    ("PTE Core", TestKind::PteCore),
    ("TEF Canada", TestKind::TefCanada),
    ("TCF Canada", TestKind::TcfCanada),
];

const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

fn english_date(s: &str) -> Option<IsoDate> {
    let c = re(r"^(\w+) (\d{1,2}), (\d{4})$").captures(s)?;
    let m = MONTHS.iter().position(|m| *m == &c[1])? + 1;
    IsoDate::from_ymd(
        c[3].parse().ok()?,
        u8::try_from(m).ok()?,
        c[2].parse().ok()?,
    )
}

/// (taken_from, taken_to) from a summary like "If you took your test after
/// December 10, 2023". "After"/"before" are exclusive; "between" inclusive.
fn date_range(s: &str) -> Option<(Option<IsoDate>, Option<IsoDate>)> {
    const D: &str = r"(\w+ \d{1,2}, \d{4})";
    if let Some(c) = re(&format!(r"^If you took your test after {D}$")).captures(s) {
        return Some((Some(english_date(&c[1])?.next_day()), None));
    }
    if let Some(c) = re(&format!(r"^If you took your test before {D}$")).captures(s) {
        return Some((None, Some(english_date(&c[1])?.prev_day())));
    }
    let c = re(&format!(r"^If you took your test between {D} and {D}$")).captures(s)?;
    Some((Some(english_date(&c[1])?), Some(english_date(&c[2])?)))
}

/// Raw score in applicant units: IELTS in tenths, others as integers.
fn score(test: TestKind, s: &str) -> Option<u16> {
    if test == TestKind::IeltsGeneral {
        let c = re(r"^(\d)\.(0|5)$").captures(s)?;
        return Some(c[1].parse::<u16>().ok()? * 10 + c[2].parse::<u16>().ok()?);
    }
    (!s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())).then(|| s.parse().ok())?
}

fn band(test: TestKind, s: &str) -> Option<ScoreBand> {
    match s.split_once('-') {
        Some((a, b)) => {
            let (min, max) = (score(test, a.trim())?, score(test, b.trim())?);
            (min <= max).then_some(ScoreBand {
                min,
                max: Some(max),
            })
        }
        None => Some(ScoreBand {
            min: score(test, s)?,
            max: None,
        }),
    }
}

fn parse_table(page: &Page, t: &Table, test: TestKind) -> Result<EquivalencyTable, ParseError> {
    let what = "language equivalency";
    t.expect_cols(what, 5)?;
    let bad_header = |col: usize| ParseError::BadLabel {
        what,
        table: t.index,
        row: 0,
        text: t.h(col).to_owned(),
    };
    if !matches!(t.h(0), "CLB level" | "NCLC level") {
        return Err(bad_header(0));
    }
    // Map header names to skills; each skill exactly once.
    let mut cols: [Option<usize>; 4] = [None; 4];
    for col in 1..5 {
        let skill = match t.h(col) {
            "Speaking" => Skill::Speaking,
            "Listening" => Skill::Listening,
            "Reading" => Skill::Reading,
            "Writing" => Skill::Writing,
            _ => return Err(bad_header(col)),
        };
        let slot = &mut cols[Skill::ALL.iter().position(|s| *s == skill).unwrap_or(0)];
        if slot.replace(col).is_some() {
            return Err(bad_header(col));
        }
    }
    let cols = cols.map(|c| c.unwrap_or(0));

    let mut rows = Vec::new();
    for (row, cells) in t.body() {
        let err = |text: &str| ParseError::BadNumber {
            what,
            table: t.index,
            row,
            text: text.to_owned(),
        };
        let clb: u8 = cells[0].parse().map_err(|_| err(&cells[0]))?;
        let b = |i: usize| band(test, &cells[cols[i]]).ok_or_else(|| err(&cells[cols[i]]));
        rows.push(EquivalencyRow {
            clb,
            speaking: b(0)?,
            listening: b(1)?,
            reading: b(2)?,
            writing: b(3)?,
            cite: t.cite(&page.source, row),
        });
    }
    rows.sort_by_key(|r| r.clb);

    // Consecutive CLB levels; per skill, thresholds strictly increasing and
    // ranges contiguous where an upper bound is given.
    let step = if test == TestKind::IeltsGeneral { 5 } else { 1 };
    for w in rows.windows(2) {
        let (lo, hi) = (&w[0], &w[1]);
        let inconsistent = |detail: String| ParseError::Inconsistent { what, detail };
        if hi.clb != lo.clb + 1 {
            return Err(inconsistent(format!(
                "{test:?}: CLB {} then {}",
                lo.clb, hi.clb
            )));
        }
        for skill in Skill::ALL {
            let (a, b) = (lo.band(skill), hi.band(skill));
            let contiguous = a.max.is_none_or(|m| m + step == b.min);
            if b.min <= a.min || !contiguous {
                return Err(inconsistent(format!(
                    "{test:?} {skill:?}: CLB {} {a:?} then {b:?}",
                    lo.clb
                )));
            }
        }
    }

    let (taken_from, taken_to) = match t.context.get(1) {
        None => (None, None),
        Some(s) => date_range(s).ok_or_else(|| ParseError::BadLabel {
            what,
            table: t.index,
            row: 0,
            text: s.clone(),
        })?,
    };
    Ok(EquivalencyTable {
        test,
        taken_from,
        taken_to,
        rows,
        cite: t.cite(&page.source, 0),
    })
}

pub fn parse_equivalency(page: &Page) -> Result<Vec<EquivalencyTable>, ParseError> {
    let what = "language equivalency";
    let mut out = Vec::new();
    for t in &page.tables {
        let Some(name) = t.context.first() else {
            continue;
        };
        let Some((_, test)) = TESTS.iter().find(|(n, _)| n == name) else {
            return Err(ParseError::BadLabel {
                what,
                table: t.index,
                row: 0,
                text: name.clone(),
            });
        };
        if t.context.len() > 2 {
            return Err(ParseError::BadLabel {
                what,
                table: t.index,
                row: 0,
                text: t.context.join(" > "),
            });
        }
        out.push(parse_table(page, t, *test)?);
    }
    // Every test present; dated tables of one test must not overlap.
    for (name, test) in TESTS {
        let tables: Vec<_> = out.iter().filter(|t| t.test == *test).collect();
        if tables.is_empty() {
            return Err(ParseError::TableNotFound(name));
        }
        for (i, a) in tables.iter().enumerate() {
            for b in &tables[i + 1..] {
                let overlap = a
                    .taken_from
                    .as_ref()
                    .is_none_or(|f| b.taken_to.as_ref().is_none_or(|t| f <= t))
                    && b.taken_from
                        .as_ref()
                        .is_none_or(|f| a.taken_to.as_ref().is_none_or(|t| f <= t));
                if overlap {
                    return Err(ParseError::Inconsistent {
                        what,
                        detail: format!("{name}: overlapping date ranges"),
                    });
                }
            }
        }
    }
    Ok(out)
}
