#![allow(clippy::unwrap_used)] // test code may panic

//! Offline: the watcher, fed the saved official pages, must rebuild exactly
//! the shipped grids (no changes), and must report a mutated value.

use std::collections::BTreeMap;

use rules_engine::grid::Grid;
use rules_watcher::{Kind, SOURCES, build_candidate, fingerprint, grid_changes, parse_draws};

const DIR: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../rules-engine/fixtures/sources"
);

fn file_for(id: &str) -> &'static str {
    match id {
        "fsw" => "travailleurs-qualifies-federal.html",
        "cec" => "categorie-experience-canadienne.html",
        "fst" => "metiers-specialises.html",
        "preuve-fonds" => "preuve-fonds-suffisants.html",
        "ee-rounds" => "ee_rounds_123_fr.json",
        "scg-outil-udit" => "scg-outil-udit.js",
        "criteries-scg" => "criteries-scg.html",
        "salaire-eleve" => "salaire-eleve.html",
        "modifications" => "modifications.html",
        "equivalences-aip-en" => "equivalences-aip-en.html",
        "examen-linguistique" => "examen-linguistique.html",
        "evaluer-diplomes-etudes" => "evaluer-diplomes-etudes.html",
        "verifier-note" => "verifier-note.html",
        other => panic!("{other}"),
    }
}

fn saved() -> BTreeMap<&'static str, Vec<u8>> {
    SOURCES
        .iter()
        .map(|s| {
            (
                s.id,
                std::fs::read(format!("{DIR}/{}", file_for(s.id))).unwrap(),
            )
        })
        .collect()
}

fn v1() -> Grid {
    serde_json::from_str(include_str!("../../rules-engine/grids/crs-2026-11-03.json")).unwrap()
}

fn parts(g: &Grid) -> serde_json::Value {
    let mut crs = serde_json::to_value(&g.crs).unwrap();
    crs.as_object_mut().unwrap().remove("labour_market");
    crs.as_object_mut().unwrap().remove("high_wage");
    serde_json::json!({
        "crs": crs,
        "language": g.language,
        "programs": g.programs,
        "labour_market": g.crs.labour_market,
        "high_wage": g.crs.high_wage,
    })
}

fn candidate_parts(bodies: &BTreeMap<&'static str, Vec<u8>>) -> serde_json::Value {
    let c = build_candidate(bodies);
    serde_json::json!({
        "crs": c.crs.unwrap(),
        "language": c.language.unwrap(),
        "programs": c.programs.unwrap(),
        "labour_market": c.labour_market.unwrap(),
        "high_wage": c.high_wage.unwrap(),
    })
}

#[test]
fn saved_pages_rebuild_the_shipped_grid_exactly() {
    let changes = grid_changes(&parts(&v1()), &candidate_parts(&saved()));
    assert!(changes.is_empty(), "{changes:#?}");
}

#[test]
fn a_changed_value_is_reported_with_its_source_row() {
    let mut bodies = saved();
    let html = String::from_utf8(bodies["criteries-scg"].clone()).unwrap();
    let at = html.find("<td>18 ans</td>").unwrap();
    let rel = html[at..].find(">90<").unwrap();
    let mut mutated = html.clone();
    mutated.replace_range(at + rel..at + rel + 4, ">91<");
    bodies.insert("criteries-scg", mutated.into_bytes());

    let changes = grid_changes(&parts(&v1()), &candidate_parts(&bodies));
    assert_eq!(changes.len(), 1, "{changes:#?}");
    let c = &changes[0];
    assert!(
        c.path.contains("age[") && c.path.ends_with("with_spouse"),
        "{}",
        c.path
    );
    assert_eq!(
        (c.before.as_deref(), c.after.as_deref()),
        (Some("90"), Some("91"))
    );
    assert!(c.source_text.as_deref().unwrap().starts_with("18 ans | 91"));
}

#[test]
fn a_redesigned_page_fails_only_its_part() {
    let mut bodies = saved();
    bodies.insert(
        "salaire-eleve",
        b"<html><main><p>Page en maintenance</p></main></html>".to_vec(),
    );
    let c = build_candidate(&bodies);
    assert!(c.high_wage.is_err());
    assert!(c.crs.is_ok() && c.programs.is_ok() && c.language.is_ok() && c.labour_market.is_ok());
}

#[test]
fn fingerprints_ignore_per_request_tokens() {
    let src = SOURCES.iter().find(|s| s.id == "criteries-scg").unwrap();
    let html = String::from_utf8(saved()["criteries-scg"].clone()).unwrap();
    let a = fingerprint(src, html.as_bytes());
    // Akamai beacon tokens live outside <main>.
    let b = fingerprint(
        src,
        html.replacen(
            "<head>",
            "<head><script>var ak='random-token-123';</script>",
            1,
        )
        .as_bytes(),
    );
    assert_eq!(a, b);
    let c = fingerprint(
        src,
        html.replacen("<td>18 ans</td>", "<td>18 ans révisé</td>", 1)
            .as_bytes(),
    );
    assert_ne!(a, c);
}

#[test]
fn draws_feed() {
    let draws = parse_draws(&saved()["ee-rounds"]).unwrap();
    assert!(draws.len() > 400);
    let latest = &draws[0];
    assert_eq!(
        (
            latest.number.as_str(),
            latest.date.as_str(),
            latest.invitations,
            latest.min_crs
        ),
        ("450", "2026-10-16", 5000, 435)
    );
    assert!(draws.windows(2).all(|w| w[0].date >= w[1].date));
    // Two draws on the same day, published as 91a and 91b.
    assert!(draws.iter().any(|d| d.number == "91a") && draws.iter().any(|d| d.number == "91b"));
    assert!(parse_draws(br#"{"rounds":[{"drawNumber":"1","drawDate":"2026-01-01","drawName":"x","drawSize":"10","drawCRS":"5000"}]}"#).is_err(), "CRS > 1200 refused");
}

#[test]
fn every_source_is_allowlisted_https() {
    let allow = net::Allowlist::new(["www.canada.ca", "ircc.canada.ca"]);
    for s in SOURCES {
        assert!(
            allow.check(&url::Url::parse(s.url).unwrap()).is_ok(),
            "{}",
            s.url
        );
    }
    assert_eq!(SOURCES.iter().filter(|s| s.kind == Kind::Grid).count(), 9);
}

/// Live: fetch every official page now and compare with the shipped grid.
/// Run with `--include-ignored`; prints the changes (an empty list = IRCC
/// pages still match the approved grid).
#[tokio::test]
#[ignore = "network"]
async fn live_check_against_shipped_grid() {
    let sink = std::sync::Arc::new(audit::MemorySink::default());
    let client = net::Client::new(
        net::Allowlist::new(["www.canada.ca", "ircc.canada.ca"]),
        sink.clone(),
    )
    .unwrap();
    let fetched = rules_watcher::fetch_all(&client).await;
    let mut bodies = BTreeMap::new();
    for f in &fetched {
        match &f.result {
            Ok(b) => {
                bodies.insert(f.id, b.clone());
            }
            Err(e) => panic!("{}: {e}", f.id),
        }
    }
    assert_eq!(sink.records().len(), SOURCES.len(), "every fetch audited");
    let changes = grid_changes(&parts(&v1()), &candidate_parts(&bodies));
    println!("live changes vs shipped grid: {changes:#?}");
    let saved = saved();
    for s in SOURCES {
        let same = fingerprint(s, &saved[s.id]) == fingerprint(s, &bodies[s.id]);
        println!(
            "{:<26} {}",
            s.id,
            if same { "unchanged" } else { "CHANGED" }
        );
    }
}

proptest::proptest! {
    #![proptest_config(proptest::prelude::ProptestConfig { cases: 256, ..Default::default() })]

    /// Hostile or corrupted draws feeds are refused, never panic.
    #[test]
    fn draws_feed_never_panics(bytes in proptest::collection::vec(proptest::prelude::any::<u8>(), 0..2000)) {
        let _ = parse_draws(&bytes);
    }

    #[test]
    fn draws_rows_with_hostile_values(number in "\\PC{0,12}", crs in "\\PC{0,24}", size in "\\PC{0,12}") {
        let feed = serde_json::json!({ "rounds": [{ "drawNumber": number, "drawDate": "2026-10-16", "drawName": "x", "drawSize": size, "drawCRS": crs }] });
        if let Ok(d) = parse_draws(feed.to_string().as_bytes()) {
            proptest::prop_assert!(d[0].min_crs <= 1200);
            proptest::prop_assert!(d[0].number.chars().all(|c| c.is_ascii_alphanumeric()));
        }
    }
}

#[test]
fn every_source_has_a_content_type_allowlist() {
    for s in SOURCES {
        assert!(!rules_watcher::accepted_types(s).is_empty(), "{}", s.id);
    }
}

/// The snapshot published for the web app carries every source and parses
/// back to the same candidate (it crosses CI -> site -> browser as JSON).
#[test]
fn published_snapshot_round_trips() {
    let fetched: Vec<rules_watcher::Fetched> = saved()
        .into_iter()
        .map(|(id, body)| rules_watcher::Fetched {
            id,
            result: Ok(body),
        })
        .chain([rules_watcher::Fetched {
            id: "verifier-note",
            result: Err("HTTP 503".into()),
        }])
        .collect();
    let p = rules_watcher::publish(&fetched, "2026-10-21T00:00:00Z".into());
    assert_eq!(p.sources.len(), SOURCES.len() + 1);
    let modifications = p.sources.iter().find(|s| s.id == "modifications").unwrap();
    assert!(
        modifications
            .main_text
            .as_ref()
            .is_some_and(|t| t.len() > 100)
    );
    assert!(
        p.sources
            .iter()
            .any(|s| s.error.as_deref() == Some("HTTP 503"))
    );
    assert!(p.draws.as_ref().is_ok_and(|d| !d.is_empty()));

    let json = serde_json::to_string(&p).unwrap();
    let back: rules_watcher::Published = serde_json::from_str(&json).unwrap();
    let c = back.candidate;
    let rebuilt = serde_json::json!({
        "crs": c.crs.unwrap(),
        "language": c.language.unwrap(),
        "programs": c.programs.unwrap(),
        "labour_market": c.labour_market.unwrap(),
        "high_wage": c.high_wage.unwrap(),
    });
    assert!(grid_changes(&parts(&v1()), &rebuilt).is_empty());
}
