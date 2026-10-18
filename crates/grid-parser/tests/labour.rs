#![allow(clippy::unwrap_used)] // test helpers may panic

//! Labour market factor and high-wage list (in force 2026-11-03).

use grid_parser::{Page, ParseError, parse_high_wage, parse_labour_market};
use rules_engine::grid::{Grid, WageTier};

const MODIF: &str = include_str!("../../rules-engine/fixtures/sources/modifications.html");
const WAGES: &str = include_str!("../../rules-engine/fixtures/sources/salaire-eleve.html");
const V1: &str = include_str!("../../rules-engine/grids/crs-2026-11-03.json");

fn v1() -> Grid {
    serde_json::from_str(V1).unwrap()
}

fn swap(page: &str, from: &str, to: &str) -> String {
    assert_eq!(
        page.matches(from).count(),
        1,
        "anchor must be unique: {from:?}"
    );
    page.replacen(from, to, 1)
}

#[test]
fn saved_pages_match_committed_grid() {
    let g = v1();
    let lm = parse_labour_market(&Page::parse("modifications", MODIF).unwrap()).unwrap();
    assert_eq!(Some(lm), g.crs.labour_market);
    let hw = parse_high_wage(&Page::parse("salaire-eleve", WAGES).unwrap()).unwrap();
    assert_eq!(Some(hw), g.crs.high_wage);
}

#[test]
fn duration_points_match_criteria_page() {
    let g = v1();
    let lm = g.crs.labour_market.unwrap();
    let a: Vec<_> = lm.duration.iter().map(|r| (r.key, r.points)).collect();
    let b: Vec<_> = g
        .crs
        .canadian_work
        .iter()
        .map(|r| (r.key, r.points))
        .collect();
    assert_eq!(a, b);
}

#[test]
fn tiers_lookup() {
    let hw = v1().crs.high_wage.unwrap();
    assert_eq!(hw.tier("00011"), Some(WageTier::AtLeast2));
    assert_eq!(hw.tier("11100"), Some(WageTier::AtLeast1_3));
    assert_eq!(hw.tier("99999"), None);
}

#[test]
fn summary_must_agree_with_detail_tables() {
    let html = MODIF.replacen("140 points", "141 points", 1);
    let e = parse_labour_market(&Page::parse("modifications", &html).unwrap()).unwrap_err();
    assert!(
        matches!(
            e,
            ParseError::Inconsistent {
                what: "labour market: summary",
                ..
            }
        ),
        "{e:?}"
    );
}

#[test]
fn duplicate_or_malformed_noc_fails() {
    for (from, to) in [
        ("<td>11100</td>", "<td>00011</td>"),
        ("<td>11100</td>", "<td>1110</td>"),
    ] {
        let html = swap(WAGES, from, to);
        let e = parse_high_wage(&Page::parse("salaire-eleve", &html).unwrap()).unwrap_err();
        assert!(
            matches!(
                e,
                ParseError::BadLabel {
                    what: "high-wage occupations",
                    ..
                }
            ),
            "{e:?}"
        );
    }
}

#[test]
fn missing_effective_date_fails() {
    let html = WAGES.replace("Entrée en vigueur le", "En vigueur dès le");
    let e = parse_high_wage(&Page::parse("salaire-eleve", &html).unwrap()).unwrap_err();
    assert_eq!(e, ParseError::TextNotFound("high-wage list effective date"));
}
