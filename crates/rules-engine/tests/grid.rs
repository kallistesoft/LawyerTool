#![allow(clippy::unwrap_used)] // test helpers may panic

use rules_engine::grid::{Grid, GridIssue, IsoDate, in_force};

const V0: &str = include_str!("../grids/crs-2026-10-16.json");

fn v0() -> Grid {
    serde_json::from_str(V0).unwrap()
}

#[test]
fn v0_only_issue_is_second_language_clb4() {
    // D-002: the criteria page skips NCLC 4 for the second official language.
    assert_eq!(
        v0().issues(),
        vec![GridIssue::Gap {
            table: "second_language",
            value: 4
        }]
    );
}

#[test]
fn picks_grid_in_force() {
    let a = v0();
    let mut b = v0();
    b.id = "crs-2026-11-03".into();
    b.effective_from = IsoDate::parse("2026-11-03").unwrap();
    let grids = [a, b];
    let at = |d| in_force(&grids, &IsoDate::parse(d).unwrap()).map(|g| g.id.as_str());
    assert_eq!(at("2026-10-15"), None);
    assert_eq!(at("2026-10-16"), Some("crs-2026-10-16"));
    assert_eq!(at("2026-11-02"), Some("crs-2026-10-16"));
    assert_eq!(at("2026-11-03"), Some("crs-2026-11-03"));
}

#[test]
fn iso_dates() {
    assert!(IsoDate::parse("2026-11-03").is_some());
    for bad in [
        "2026-13-01",
        "2026-1-01",
        "20261103",
        "2026-11-3x",
        "2026-00-10",
    ] {
        assert!(IsoDate::parse(bad).is_none(), "{bad}");
    }
}

#[test]
fn rulings_resolve_all_v0_issues() {
    let mut g = v0();
    rules_engine::rulings::apply(&mut g);
    assert_eq!(g.issues(), vec![]);
    let row = g
        .crs
        .second_language
        .iter()
        .find(|r| r.key.contains(4))
        .unwrap();
    assert_eq!(row.points.get(true), 0);
    assert_eq!(row.cite.source, "ruling:D-002");

    let once = g.clone();
    rules_engine::rulings::apply(&mut g);
    assert_eq!(g, once, "rulings are idempotent");
}
