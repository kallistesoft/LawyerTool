#![allow(clippy::unwrap_used)] // test helpers may panic

use grid_parser::{Page, ParseError, ProgramPages, parse_programs};
use rules_engine::grid::Grid;
use rules_engine::programs::Programs;

macro_rules! src {
    ($f:literal) => {
        include_str!(concat!("../../rules-engine/fixtures/sources/", $f))
    };
}
const V0: &str = include_str!("../../rules-engine/grids/crs-2026-10-16.json");

struct Html {
    fsw: String,
    cec: String,
    fst: String,
    language: String,
    education: String,
    funds: String,
}

fn html() -> Html {
    Html {
        fsw: src!("travailleurs-qualifies-federal.html").into(),
        cec: src!("categorie-experience-canadienne.html").into(),
        fst: src!("metiers-specialises.html").into(),
        language: src!("examen-linguistique.html").into(),
        education: src!("evaluer-diplomes-etudes.html").into(),
        funds: src!("preuve-fonds-suffisants.html").into(),
    }
}

fn parse(h: &Html) -> Result<Programs, ParseError> {
    let p = |id, s: &str| Page::parse(id, s).unwrap();
    let (fsw, cec, fst) = (p("fsw", &h.fsw), p("cec", &h.cec), p("fst", &h.fst));
    let (language, education, funds) = (
        p("examen-linguistique", &h.language),
        p("evaluer-diplomes-etudes", &h.education),
        p("preuve-fonds", &h.funds),
    );
    parse_programs(&ProgramPages {
        fsw: &fsw,
        cec: &cec,
        fst: &fst,
        language: &language,
        education: &education,
        funds: &funds,
    })
}

fn swap(s: &mut String, from: &str, to: &str) {
    assert_eq!(
        s.matches(from).count(),
        1,
        "anchor must be unique: {from:?}"
    );
    *s = s.replacen(from, to, 1);
}

#[test]
fn saved_pages_match_committed_grid() {
    let committed: Grid = serde_json::from_str(V0).unwrap();
    assert_eq!(Some(parse(&html()).unwrap()), committed.programs);
}

#[test]
fn fst_trades_membership() {
    let p = parse(&html()).unwrap();
    let is_trade = |noc: &str| p.fst.trades.iter().any(|g| g.contains(noc));
    assert!(is_trade("72100"), "machinists, major group 72");
    assert!(!is_trade("72600"), "sub-major group 726 is excluded");
    assert!(is_trade("63200"), "sub-group 6320 (cooks)");
    assert!(is_trade("62200"), "unit group 62200 (chefs)");
    assert!(!is_trade("62201"));
    assert!(!is_trade("21231"));
}

#[test]
fn funds_beyond_table() {
    let p = parse(&html()).unwrap();
    assert_eq!(p.funds.required(1).unwrap().0, 15_263);
    assert_eq!(p.funds.required(9).unwrap().0, 40_392 + 2 * 4_112);
    assert!(p.funds.required(0).is_none());
}

#[test]
fn changed_threshold_is_picked_up() {
    let mut h = html();
    swap(
        &mut h.fsw,
        "Si vous obtenez <strong>67 points ou plus",
        "Si vous obtenez <strong>70 points ou plus",
    );
    assert_eq!(parse(&h).unwrap().fsw.pass_mark.value, 70);
}

#[test]
fn language_pages_must_agree() {
    // FSW page says "below NCLC 7: not eligible"; the language page must agree.
    let mut h = html();
    swap(&mut h.language, "<td>NCLC7</td>", "<td>NCLC 8</td>");
    let e = parse(&h).unwrap_err();
    assert!(matches!(e, ParseError::Inconsistent { .. }), "{e:?}");
}

#[test]
fn malformed_funds_fail() {
    let mut h = html();
    swap(&mut h.funds, "15&nbsp;263&nbsp;$", "15&nbsp;263,50&nbsp;$");
    assert!(matches!(
        parse(&h).unwrap_err(),
        ParseError::BadNumber {
            what: "proof of funds",
            ..
        }
    ));
}
