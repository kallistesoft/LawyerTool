#![allow(clippy::unwrap_used)] // test helpers may panic

use applicant::{IsoDate, LanguageTest, Skill, TestKind};
use regex::Regex;
use rules_engine::grid::Grid;
use rules_engine::language::{LanguageError, table, to_clb, valid_on};

const V0: &str = include_str!("../grids/crs-2026-10-16.json");
const CALC: &str = include_str!("../fixtures/sources/scg-outil-udit.js");

fn grid() -> Grid {
    serde_json::from_str(V0).unwrap()
}

fn d(s: &str) -> IsoDate {
    IsoDate::parse(s).unwrap()
}

fn test(kind: TestKind, date: &str, [s, l, r, w]: [u16; 4]) -> LanguageTest {
    LanguageTest {
        test: kind,
        test_date: d(date),
        speaking: s,
        listening: l,
        reading: r,
        writing: w,
    }
}

fn one(kind: TestKind, date: &str, skill: Skill, score: u16) -> Result<u8, LanguageError> {
    let mut scores = [0; 4];
    scores[Skill::ALL.iter().position(|s| *s == skill).unwrap()] = score;
    // Fill the other skills with each table's lowest valid score.
    let g = grid();
    let t = table(&g, kind, &d(date)).unwrap();
    for (i, s) in Skill::ALL.iter().enumerate() {
        if *s != skill {
            scores[i] = t.rows[0].band(*s).min;
        }
    }
    to_clb(&g, &test(kind, date, scores)).map(|(c, _)| c.get(skill))
}

/// Every row boundary of every table: min maps to the row, the score just
/// below maps to the previous level, max (when given) maps to the row.
#[test]
fn every_boundary() {
    let g = grid();
    let mut checked = 0;
    for t in &g.language {
        let date = t
            .taken_from
            .clone()
            .or(t.taken_to.clone())
            .unwrap_or(d("2026-01-01"));
        let step = if t.test == TestKind::IeltsGeneral {
            5
        } else {
            1
        };
        for row in &t.rows {
            for skill in Skill::ALL {
                let b = row.band(skill);
                assert_eq!(
                    one(t.test, date.as_str(), skill, b.min),
                    Ok(row.clb),
                    "{:?} {skill:?} {b:?}",
                    t.test
                );
                if b.min >= step {
                    assert_eq!(
                        one(t.test, date.as_str(), skill, b.min - step),
                        Ok(row.clb - 1)
                    );
                }
                if let Some(m) = b.max {
                    assert_eq!(one(t.test, date.as_str(), skill, m), Ok(row.clb));
                }
                checked += 1;
            }
        }
    }
    assert_eq!(
        checked,
        (6 * 7 + 8) * 4,
        "(6 tables × 7 rows + PTE's 8 rows) × 4 skills"
    );
}

#[test]
fn tef_table_by_test_date() {
    let g = grid();
    let from = |date: &str| table(&g, TestKind::TefCanada, &d(date)).map(|t| t.taken_from.clone());
    assert_eq!(from("2024-05-01"), Ok(Some(d("2023-12-11"))));
    assert_eq!(from("2023-12-11"), Ok(Some(d("2023-12-11"))));
    assert_eq!(
        from("2023-12-10"),
        Ok(Some(d("2019-10-01"))),
        "D-003: older scale"
    );
    assert_eq!(from("2019-10-01"), Ok(Some(d("2019-10-01"))));
    assert_eq!(from("2019-09-29"), Ok(None));
    assert!(
        matches!(from("2019-09-30"), Err(LanguageError::NoTable { .. })),
        "D-003: refused"
    );

    // The same raw score means different levels on different scales.
    let s = [460, 440, 440, 430];
    let new = to_clb(&g, &test(TestKind::TefCanada, "2025-03-01", s))
        .unwrap()
        .0;
    let mid = to_clb(&g, &test(TestKind::TefCanada, "2021-03-01", s))
        .unwrap()
        .0;
    assert_eq!((new.min(), mid.min()), (7, 6));
}

#[test]
fn above_scale_is_an_error() {
    // Old TEF scale: speaking tops out at 450.
    let e = to_clb(
        &grid(),
        &test(TestKind::TefCanada, "2018-06-01", [460, 300, 250, 400]),
    )
    .unwrap_err();
    assert_eq!(
        e,
        LanguageError::OutOfScale {
            test: TestKind::TefCanada,
            skill: Skill::Speaking,
            score: 460
        }
    );
}

#[test]
fn ielts_examples() {
    let g = grid();
    let c = to_clb(
        &g,
        &test(TestKind::IeltsGeneral, "2025-01-01", [70, 80, 70, 70]),
    )
    .unwrap()
    .0;
    assert_eq!(
        (c.speaking, c.listening, c.reading, c.writing),
        (9, 9, 9, 9)
    );
    let c = to_clb(
        &g,
        &test(TestKind::IeltsGeneral, "2025-01-01", [65, 70, 60, 55]),
    )
    .unwrap()
    .0;
    assert_eq!(
        (c.speaking, c.listening, c.reading, c.writing),
        (8, 7, 7, 6)
    );
    let c = to_clb(
        &g,
        &test(TestKind::IeltsGeneral, "2025-01-01", [35, 40, 30, 35]),
    )
    .unwrap()
    .0;
    assert_eq!(c.min(), 3, "below CLB 4");
}

#[test]
fn validity() {
    let t = test(TestKind::CelpipGeneral, "2024-10-18", [9, 9, 9, 9]);
    assert!(valid_on(&t, &d("2026-10-18")));
    assert!(!valid_on(&t, &d("2026-10-19")));
    assert!(!valid_on(&t, &d("2024-10-17")), "not taken yet");
}

/// Cross-check against the calculator's own arrays: the lowest score of each
/// calculator row must convert to that row's CLB. TEF uses the calculator's
/// (pre-2019-09-30) scale.
#[test]
fn agrees_with_calculator_tables() {
    let row = Regex::new(
        r#"\[\s*"([A-H])",\s*"([^"]*)",\s*"([^"]*)",\s*"([^"]*)",\s*"([^"]*)",\s*"(\d+)""#,
    )
    .unwrap();
    let first = Regex::new(r"(\d+(?:\s*\.\s*\d)?)").unwrap();
    let mut checked = 0;
    for (var, kind, date) in [
        ("celpip", TestKind::CelpipGeneral, "2025-01-01"),
        ("ielts", TestKind::IeltsGeneral, "2025-01-01"),
        ("pte", TestKind::PteCore, "2025-01-01"),
        ("tef", TestKind::TefCanada, "2018-01-01"),
        ("tcf", TestKind::TcfCanada, "2025-01-01"),
    ] {
        let start = CALC.find(&format!("var {var} = [")).unwrap();
        let body = &CALC[start..start + CALC[start..].find("];").unwrap()];
        for c in row.captures_iter(body) {
            let clb: u8 = c[6].parse().unwrap();
            // Calculator order: speaking, listening, reading, writing.
            for (i, skill) in Skill::ALL.iter().enumerate() {
                let cell = &c[2 + i];
                let n = first.captures(cell).unwrap()[1].replace(' ', "");
                let score = if kind == TestKind::IeltsGeneral {
                    (n.parse::<f32>().unwrap() * 10.0).round() as u16
                } else {
                    n.parse().unwrap()
                };
                let got = one(kind, date, *skill, score).unwrap();
                if clb == 0 {
                    assert!(
                        got < 4,
                        "{var} {skill:?} {cell:?}: calculator 'below 4', got {got}"
                    );
                } else {
                    assert_eq!(got, clb, "{var} {skill:?} {cell:?}");
                }
                checked += 1;
            }
        }
    }
    assert_eq!(checked, 5 * 8 * 4);
}
