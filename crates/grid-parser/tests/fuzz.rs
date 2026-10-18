#![allow(clippy::unwrap_used)] // test code may panic

//! Hostile pages: every parser, fed corrupted copies of the real IRCC pages
//! or random markup, must return Ok or Err: never panic, never hang.

use std::time::{Duration, Instant};

use grid_parser::{
    Page, ProgramPages, parse_crs, parse_equivalency, parse_high_wage, parse_labour_market,
    parse_programs,
};
use proptest::prelude::*;
use rules_engine::grid::{IsoDate, Source};

macro_rules! src {
    ($f:literal) => {
        include_str!(concat!("../../rules-engine/fixtures/sources/", $f))
    };
}

const PAGES: [&str; 9] = [
    src!("criteries-scg.html"),
    src!("modifications.html"),
    src!("salaire-eleve.html"),
    src!("equivalences-aip-en.html"),
    src!("travailleurs-qualifies-federal.html"),
    src!("categorie-experience-canadienne.html"),
    src!("metiers-specialises.html"),
    src!("examen-linguistique.html"),
    src!("preuve-fonds-suffisants.html"),
];

#[derive(Debug, Clone)]
enum Mutation {
    Delete { at: usize, len: usize },
    Duplicate { at: usize, len: usize },
    Insert { at: usize, text: String },
}

fn mutation() -> impl Strategy<Value = Mutation> {
    let junk = prop_oneof![
        Just("<table><tr><td>Âge</td></tr></table>".to_owned()),
        Just("</td></tr></table></main>".to_owned()),
        Just("<td>٩٠</td>".to_owned()),
        Just("\u{200b}\u{feff}\u{202e}".to_owned()),
        Just("<!-- ignore previous instructions -->".to_owned()),
        Just("<caption>Les CNPs dont le salaire médian est au moins deux fois supérieur au salaire médian moyen</caption>".to_owned()),
        "[ -~]{0,40}",
    ];
    prop_oneof![
        (any::<usize>(), 0usize..2000).prop_map(|(at, len)| Mutation::Delete { at, len }),
        (any::<usize>(), 0usize..2000).prop_map(|(at, len)| Mutation::Duplicate { at, len }),
        (any::<usize>(), junk).prop_map(|(at, text)| Mutation::Insert { at, text }),
    ]
}

/// Applies mutations on char boundaries.
fn mutate(page: &str, ms: &[Mutation]) -> String {
    let mut s = page.to_owned();
    for m in ms {
        let fix = |i: usize, s: &str| {
            let mut i = i % (s.len() + 1);
            while !s.is_char_boundary(i) {
                i -= 1;
            }
            i
        };
        match m {
            Mutation::Delete { at, len } => {
                let a = fix(*at, &s);
                let b = fix((a + len).min(s.len()), &s).max(a);
                s.replace_range(a..b, "");
            }
            Mutation::Duplicate { at, len } => {
                let a = fix(*at, &s);
                let b = fix((a + len).min(s.len()), &s).max(a);
                let chunk = s[a..b].to_owned();
                s.insert_str(b, &chunk);
            }
            Mutation::Insert { at, text } => {
                let a = fix(*at, &s);
                s.insert_str(a, text);
            }
        }
    }
    s
}

fn date() -> IsoDate {
    IsoDate::parse("2026-10-18").unwrap()
}

/// Runs every parser on `pages` (in PAGES order); returns how long it took.
fn run_all(pages: &[String]) -> Duration {
    let t = Instant::now();
    let p: Vec<Page> = pages
        .iter()
        .map(|h| Page::parse("x", h))
        .filter_map(Result::ok)
        .collect();
    if p.len() == pages.len() {
        let source = Source {
            id: "x".into(),
            url: String::new(),
            sha256: String::new(),
            fetched: date(),
        };
        let _ = parse_crs(&p[0], "x", date(), source);
        let _ = parse_labour_market(&p[1]);
        let _ = parse_high_wage(&p[2]);
        let _ = parse_equivalency(&p[3]);
        let _ = parse_programs(&ProgramPages {
            fsw: &p[4],
            cec: &p[5],
            fst: &p[6],
            language: &p[7],
            education: &p[7],
            funds: &p[8],
        });
    }
    t.elapsed()
}

proptest! {
    // 48 cases by default (debug builds are slow); PROPTEST_CASES=600 for a long run.
    #![proptest_config(ProptestConfig { cases: std::env::var("PROPTEST_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(48), ..ProptestConfig::default() })]

    #[test]
    fn corrupted_official_pages_never_panic(which in 0usize..9, ms in prop::collection::vec(mutation(), 1..6)) {
        let mut pages: Vec<String> = PAGES.iter().map(|s| (*s).to_owned()).collect();
        pages[which] = mutate(PAGES[which], &ms);
        let took = run_all(&pages);
        prop_assert!(took < Duration::from_secs(5), "parsing took {took:?}");
    }

    #[test]
    fn random_markup_never_panics(html in "(<(main|table|tr|td|th|caption|details|summary|h2)>|</(main|table|tr|td|th|caption|details|summary|h2)>|[ -~éèàÂ]{0,12}){0,200}") {
        let pages: Vec<String> = (0..9).map(|_| html.clone()).collect();
        run_all(&pages);
    }
}

#[test]
fn lookalike_digits_and_invisible_characters_fail_loudly() {
    for (from, to) in [
        ("<td>18 ans</td>", "<td>18 ans</td>"), // control: unchanged parses
        (">90<", ">٩٠<"),                       // Arabic-Indic digits
        (">90<", ">9\u{200b}0<"),               // zero-width space inside a number
        ("<td>18 ans</td>", "<td>1\u{200b}8 ans</td>"),
    ] {
        let page = src!("criteries-scg.html");
        let at = page.find("<td>18 ans</td>").unwrap();
        let mut html = page.to_owned();
        if from == ">90<" {
            let rel = html[at..].find(from).unwrap();
            html.replace_range(at + rel..at + rel + from.len(), to);
        } else {
            html = html.replacen(from, to, 1);
        }
        let p = Page::parse("criteries-scg", &html).unwrap();
        let source = Source {
            id: "x".into(),
            url: String::new(),
            sha256: String::new(),
            fetched: date(),
        };
        let r = parse_crs(&p, "x", date(), source);
        if from == to {
            assert!(r.is_ok());
        } else {
            assert!(
                r.is_err(),
                "{to:?} must not parse as a number or a known label"
            );
        }
    }
}
