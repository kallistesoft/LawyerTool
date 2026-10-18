#![allow(clippy::unwrap_used, clippy::expect_used)] // test code may panic

//! Golden profiles recorded on the live IRCC calculator (fixtures/crs/golden).
//! The engine must equal the calculator's result plus the adjustments the
//! fixture documents for known divergences (each tied to a ruling).

use std::path::Path;

use applicant::{Applicant, IsoDate};
use rules_engine::crs::{Flag, score};
use rules_engine::grid::Grid;
use serde::Deserialize;

const V0: &str = include_str!("../grids/crs-2026-10-16.json");
/// Calculator script hash at recording time; a different hash makes the
/// fixtures stale (re-record).
const CALCULATOR_SHA: &str = "61392e6266be6d976735305982eeb72ce30296f086ebf1b5dc802a305639a65a";

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
struct Sections {
    core: i32,
    spouse: i32,
    skill: i32,
    bonus: i32,
    total: i32,
}

#[derive(Deserialize)]
struct Adjustment {
    id: String,
    #[serde(default)]
    core: i32,
    #[serde(default)]
    bonus: i32,
    #[serde(default)]
    total: i32,
}

#[derive(Deserialize)]
struct Recorded {
    on: String,
    calculator_js_sha256: String,
}

#[derive(Deserialize)]
struct Golden {
    id: String,
    applicant: Applicant,
    calculator: Option<Sections>,
    recorded: Option<Recorded>,
    adjustments: Vec<Adjustment>,
}

fn goldens() -> Vec<Golden> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/crs/golden");
    let mut out: Vec<Golden> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .map(|p| {
            serde_json::from_str(&std::fs::read_to_string(&p).unwrap())
                .unwrap_or_else(|e| panic!("{}: {e}", p.display()))
        })
        .collect();
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

#[test]
fn engine_matches_live_calculator() {
    let mut grid: Grid = serde_json::from_str(V0).unwrap();
    rules_engine::rulings::apply(&mut grid);
    let goldens = goldens();
    assert!(
        goldens.len() >= 25,
        "expected the full golden set, found {}",
        goldens.len()
    );

    let mut failures = Vec::new();
    for g in &goldens {
        let rec = g
            .recorded
            .as_ref()
            .unwrap_or_else(|| panic!("{}: not recorded yet", g.id));
        assert_eq!(
            rec.calculator_js_sha256, CALCULATOR_SHA,
            "{}: stale, re-record",
            g.id
        );
        let mut want = g
            .calculator
            .unwrap_or_else(|| panic!("{}: no calculator result", g.id));
        for adj in &g.adjustments {
            want.core += adj.core;
            want.bonus += adj.bonus;
            want.total += adj.total;
        }

        let r = score(&grid, &g.applicant, &IsoDate::parse(&rec.on).unwrap()).unwrap();
        let got = Sections {
            core: r.core.subtotal.into(),
            spouse: r.spouse.subtotal.into(),
            skill: r.transferability.subtotal.into(),
            bonus: r.additional.subtotal.into(),
            total: r.total.into(),
        };
        if got != want {
            failures.push(format!(
                "{}: engine {got:?} != calculator+adjustments {want:?}",
                g.id
            ));
        }

        // Each adjustment corresponds to a divergence the engine flags for review.
        for adj in &g.adjustments {
            let review = Flag::ManualReview {
                id: adj.id.clone(),
                reason: "",
            };
            assert!(
                r.flags.iter().any(|f| matches!((f, &review), (Flag::ManualReview { id: a, .. }, Flag::ManualReview { id: b, .. }) if a == b)),
                "{}: adjustment {} without a review flag",
                g.id,
                adj.id
            );
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
