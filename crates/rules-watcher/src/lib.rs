//! Rules watcher (brief rule 4–5; plan M2b), deterministic part.
//!
//! - Fetches the official pages through `net` (allowlist + audit), category
//!   `public_rules_fetch`. It never sees client data: this crate cannot
//!   depend on `vault` or `deid` (enforced by scripts/check-deps.sh).
//! - Fingerprints the normalized `<main>` content (canada.ca pages embed
//!   per-request tokens, so raw bytes differ on every fetch).
//! - Rebuilds a **candidate** grid with the same parsers as the shipped
//!   grids, and lists every value that differs from the approved grid. The
//!   lawyer approves or rejects; nothing changes on its own.
//! - Reads the official draws feed (deterministic JSON).
//! - Prose pages (announcements) get a sentence diff only: Claude extraction
//!   is deferred.

mod diff;
mod draws;

use std::collections::BTreeMap;

use grid_parser::{Page, ProgramPages};
use rules_engine::grid::{Crs, EquivalencyTable, HighWageList, LabourMarket};
use rules_engine::programs::Programs;
use serde::Serialize;
use sha2::{Digest, Sha256};

pub use diff::{Change, SentenceChange, grid_changes, sentence_diff};
pub use draws::{Draw, parse_draws};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// Parsed into the candidate grid.
    Grid,
    /// Draws feed (JSON).
    Draws,
    /// Fingerprint only (calculator script / form): a change means the
    /// golden profiles must be re-recorded and the import mapping checked.
    Watch,
    /// Announcement text: sentence diff for the lawyer.
    Prose,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct Source {
    pub id: &'static str,
    pub url: &'static str,
    pub kind: Kind,
}

macro_rules! ee {
    ($path:literal) => {
        concat!("https://www.canada.ca/fr/immigration-refugies-citoyennete/services/immigrer-canada/entree-express", $path)
    };
}

/// Every page the watcher reads (exact URLs; no crawling).
pub const SOURCES: &[Source] = &[
    Source {
        id: "criteries-scg",
        url: ee!("/verifier-note/criteries-scg.html"),
        kind: Kind::Grid,
    },
    Source {
        id: "salaire-eleve",
        url: ee!("/verifier-note/salaire-eleve.html"),
        kind: Kind::Grid,
    },
    Source {
        id: "modifications",
        url: ee!("/modifications.html"),
        kind: Kind::Prose,
    },
    Source {
        id: "equivalences-aip-en",
        url: "https://www.canada.ca/en/immigration-refugees-citizenship/services/immigrate-canada/atlantic-immigration/language-testing.html",
        kind: Kind::Grid,
    },
    Source {
        id: "fsw",
        url: ee!("/qui-presenter-demande/travailleurs-qualifies-federal.html"),
        kind: Kind::Grid,
    },
    Source {
        id: "cec",
        url: ee!("/qui-presenter-demande/categorie-experience-canadienne.html"),
        kind: Kind::Grid,
    },
    Source {
        id: "fst",
        url: ee!("/qui-presenter-demande/metiers-specialises.html"),
        kind: Kind::Grid,
    },
    Source {
        id: "examen-linguistique",
        url: ee!("/documents/examen-linguistique.html"),
        kind: Kind::Grid,
    },
    Source {
        id: "evaluer-diplomes-etudes",
        url: ee!("/documents/evaluer-diplomes-etudes.html"),
        kind: Kind::Grid,
    },
    Source {
        id: "preuve-fonds",
        url: ee!("/documents/preuve-fonds-suffisants.html"),
        kind: Kind::Grid,
    },
    Source {
        id: "scg-outil-udit",
        url: "https://ircc.canada.ca/francais/immigrer/qualifie/scg-outil-udit.js",
        kind: Kind::Watch,
    },
    Source {
        id: "verifier-note",
        url: ee!("/verifier-note.html"),
        kind: Kind::Watch,
    },
    Source {
        id: "ee-rounds",
        url: "https://www.canada.ca/content/dam/ircc/documents/json/ee_rounds_123_fr.json",
        kind: Kind::Draws,
    },
];

pub fn source(id: &str) -> Option<&'static Source> {
    SOURCES.iter().find(|s| s.id == id)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Content fingerprint: normalized `<main>` text for HTML pages (ignores
/// per-request tokens and layout), raw bytes for scripts and JSON.
pub fn fingerprint(src: &Source, body: &[u8]) -> String {
    let normalized = if src.url.ends_with(".html") {
        let html = String::from_utf8_lossy(body);
        match Page::parse(src.id, &html) {
            Ok(p) => p.text.into_bytes(),
            Err(_) => body.to_vec(),
        }
    } else {
        body.to_vec()
    };
    hex(&Sha256::digest(&normalized))
}

/// Normalized text of an HTML page's `<main>`, for prose diffs.
pub fn main_text(src: &Source, body: &[u8]) -> Option<String> {
    Page::parse(src.id, &String::from_utf8_lossy(body))
        .ok()
        .map(|p| p.text)
}

/// The grid parts rebuilt from fetched pages. Each part fails on its own:
/// a redesigned page blocks only what it feeds.
#[derive(Debug, Serialize)]
pub struct Candidate {
    pub crs: Result<Crs, String>,
    pub language: Result<Vec<EquivalencyTable>, String>,
    pub programs: Result<Programs, String>,
    pub labour_market: Result<LabourMarket, String>,
    pub high_wage: Result<HighWageList, String>,
}

fn page(bodies: &BTreeMap<&str, Vec<u8>>, id: &'static str) -> Result<Page, String> {
    let body = bodies.get(id).ok_or_else(|| format!("{id}: not fetched"))?;
    Page::parse(id, &String::from_utf8_lossy(body)).map_err(|e| format!("{id}: {e}"))
}

/// Rebuilds every grid part from fetched page bodies (keyed by source id).
pub fn build_candidate(bodies: &BTreeMap<&str, Vec<u8>>) -> Candidate {
    let placeholder = || applicant::IsoDate::parse("1970-01-01").unwrap_or_else(|| unreachable!());
    let crs = page(bodies, "criteries-scg").and_then(|p| {
        let src = rules_engine::grid::Source {
            id: "criteries-scg".into(),
            url: source("criteries-scg").map_or("", |s| s.url).into(),
            sha256: String::new(),
            fetched: placeholder(),
        };
        grid_parser::parse_crs(&p, "candidate", placeholder(), src)
            .map(|g| g.crs)
            .map_err(|e| format!("criteries-scg: {e}"))
    });
    let language = page(bodies, "equivalences-aip-en").and_then(|p| {
        grid_parser::parse_equivalency(&p).map_err(|e| format!("equivalences-aip-en: {e}"))
    });
    let programs = (|| {
        let (fsw, cec, fst) = (
            page(bodies, "fsw")?,
            page(bodies, "cec")?,
            page(bodies, "fst")?,
        );
        let (language, education, funds) = (
            page(bodies, "examen-linguistique")?,
            page(bodies, "evaluer-diplomes-etudes")?,
            page(bodies, "preuve-fonds")?,
        );
        grid_parser::parse_programs(&ProgramPages {
            fsw: &fsw,
            cec: &cec,
            fst: &fst,
            language: &language,
            education: &education,
            funds: &funds,
        })
        .map_err(|e| format!("program pages: {e}"))
    })();
    // The labour-market tables live in the announcement page.
    let labour_market = page(bodies, "modifications").and_then(|p| {
        grid_parser::parse_labour_market(&p).map_err(|e| format!("modifications: {e}"))
    });
    let high_wage = page(bodies, "salaire-eleve")
        .and_then(|p| grid_parser::parse_high_wage(&p).map_err(|e| format!("salaire-eleve: {e}")));
    Candidate {
        crs,
        language,
        programs,
        labour_market,
        high_wage,
    }
}

/// One fetch result.
#[derive(Debug, Clone, Serialize)]
pub struct Fetched {
    pub id: &'static str,
    pub result: Result<Vec<u8>, String>,
}

/// Fetches every source through the audited, allowlisted client.
pub async fn fetch_all(client: &net::Client) -> Vec<Fetched> {
    let mut out = Vec::new();
    for s in SOURCES {
        let result = match url::Url::parse(s.url) {
            Err(e) => Err(e.to_string()),
            Ok(u) => match client
                .send(net::Request::get(u, audit::DataCategory::PublicRulesFetch))
                .await
            {
                Ok(r) if r.status == 200 => Ok(r.body),
                Ok(r) => Err(format!("HTTP {}", r.status)),
                Err(e) => Err(e.to_string()),
            },
        };
        out.push(Fetched { id: s.id, result });
    }
    out
}
