#![allow(clippy::unwrap_used)] // test helpers may panic

use grid_parser::{Page, ParseError, parse_equivalency};
use rules_engine::grid::Grid;

const PAGE: &str = include_str!("../../rules-engine/fixtures/sources/equivalences-aip-en.html");
const V0: &str = include_str!("../../rules-engine/grids/crs-2026-10-16.json");

fn parse(html: &str) -> Result<Vec<rules_engine::grid::EquivalencyTable>, ParseError> {
    parse_equivalency(&Page::parse("equivalences-aip-en", html)?)
}

fn swap(from: &str, to: &str) -> String {
    assert_eq!(
        PAGE.matches(from).count(),
        1,
        "anchor must be unique: {from:?}"
    );
    PAGE.replacen(from, to, 1)
}

#[test]
fn saved_page_matches_committed_grid() {
    let committed: Grid = serde_json::from_str(V0).unwrap();
    assert_eq!(parse(PAGE).unwrap(), committed.language);
}

#[test]
fn overlapping_tef_dates_fail() {
    let html = swap(
        "If you took your test after December 10, 2023",
        "If you took your test after December 1, 2023",
    );
    let e = parse(&html).unwrap_err();
    assert!(matches!(e, ParseError::Inconsistent { .. }), "{e:?}");
}

#[test]
fn unknown_test_or_date_wording_fails() {
    let e = parse(&swap(
        "<summary>TCF Canada</summary>",
        "<summary>TCF Québec</summary>",
    ))
    .unwrap_err();
    assert!(matches!(e, ParseError::BadLabel { .. }), "{e:?}");
    let html = swap(
        "If you took your test before September 30, 2019",
        "If you took it before September 30, 2019",
    );
    assert!(matches!(
        parse(&html).unwrap_err(),
        ParseError::BadLabel { .. }
    ));
}

#[test]
fn non_contiguous_ranges_fail() {
    // PTE Core CLB 7 reading 60-68 → 60-67 leaves 68 unassigned (first of
    // two identical cells; the other is listening 60-70, so pick by order).
    let e = parse(&PAGE.replacen("<td>60-68</td>", "<td>60-67</td>", 1)).unwrap_err();
    assert!(matches!(e, ParseError::Inconsistent { .. }), "{e:?}");
}
