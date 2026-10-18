//! Regenerates the v0 grid snapshot from the saved official page:
//! `cargo run -p grid-parser --example regen > crates/rules-engine/grids/crs-2026-10-16.json`
//!
//! The output must be reviewed by hand against the page before committing.

use grid_parser::{Page, parse_crs};
use rules_engine::grid::{IsoDate, Source};

const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../rules-engine/fixtures/sources/criteries-scg.html"
);

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let html = std::fs::read_to_string(FIXTURE)?;
    let page = Page::parse("criteries-scg", &html)?;
    let date = |s| IsoDate::parse(s).ok_or("bad date");
    let source = Source {
        id: "criteries-scg".into(),
        url: "https://www.canada.ca/fr/immigration-refugies-citoyennete/services/immigrer-canada/entree-express/verifier-note/criteries-scg.html".into(),
        sha256: "c4a492adfa2cad679c2e797cc4391d7d2231198ad8b6e08c8483f37ad2080961".into(),
        fetched: date("2026-10-18")?,
    };
    let grid = parse_crs(&page, "crs-2026-10-16", date("2026-10-16")?, source)?;
    println!("{}", serde_json::to_string_pretty(&grid)?);
    Ok(())
}
