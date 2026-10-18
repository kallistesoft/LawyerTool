//! Regenerates grid snapshots from the saved official pages:
//! `cargo run -p grid-parser --example regen -- crates/rules-engine/grids`
//!
//! Output is the raw parser result (rulings are applied by the engine).
//! Review the diff by hand against the pages before committing.

use std::path::Path;

use grid_parser::{Page, parse_crs, parse_equivalency, parse_high_wage, parse_labour_market};
use rules_engine::grid::{Grid, IsoDate, Source};

const DIR: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../rules-engine/fixtures/sources"
);
const BASE: &str = "https://www.canada.ca/fr/immigration-refugies-citoyennete/services/immigrer-canada/entree-express";

/// (id, file, url, sha256), as recorded in `fixtures/sources/README.md`.
const SOURCES: &[(&str, &str, &str, &str)] = &[
    (
        "criteries-scg",
        "criteries-scg.html",
        "/verifier-note/criteries-scg.html",
        "c4a492adfa2cad679c2e797cc4391d7d2231198ad8b6e08c8483f37ad2080961",
    ),
    (
        "modifications",
        "modifications.html",
        "/modifications.html",
        "9729a26925f1ceed605766636b212c0ae1a6785ed0b638667dc1ad63e155a718",
    ),
    (
        "salaire-eleve",
        "salaire-eleve.html",
        "/verifier-note/salaire-eleve.html",
        "a7a15b38f9ab6ce6cc977046105b92516ed0def3153f7abda76621beda06238c",
    ),
    (
        "equivalences-aip-en",
        "equivalences-aip-en.html",
        "https://www.canada.ca/en/immigration-refugees-citizenship/services/immigrate-canada/atlantic-immigration/language-testing.html",
        "cdbc513a16992dd2cd0b4c6ea0edde725b2195096ac28928d12b5d2246846fa4",
    ),
];

type Res<T> = Result<T, Box<dyn std::error::Error>>;

fn date(s: &str) -> Res<IsoDate> {
    Ok(IsoDate::parse(s).ok_or("bad date")?)
}

fn load(id: &str) -> Res<(Page, Source)> {
    let (_, file, path, sha) = SOURCES.iter().find(|s| s.0 == id).ok_or("unknown source")?;
    let html = std::fs::read_to_string(Path::new(DIR).join(file))?;
    let source = Source {
        id: id.into(),
        url: if path.starts_with("https://") {
            (*path).to_owned()
        } else {
            format!("{BASE}{path}")
        },
        sha256: (*sha).into(),
        fetched: date("2026-10-18")?,
    };
    Ok((Page::parse(id, &html)?, source))
}

fn write(dir: &Path, g: &Grid) -> Res<()> {
    std::fs::write(
        dir.join(format!("{}.json", g.id)),
        serde_json::to_string_pretty(g)? + "\n",
    )?;
    Ok(())
}

fn main() -> Res<()> {
    let dir = std::env::args().nth(1).ok_or("usage: regen <output dir>")?;
    let dir = Path::new(&dir);

    let (page, source) = load("criteries-scg")?;
    let mut v0 = parse_crs(&page, "crs-2026-10-16", date("2026-10-16")?, source)?;
    let (equiv, equiv_src) = load("equivalences-aip-en")?;
    v0.language = parse_equivalency(&equiv)?;
    v0.sources.push(equiv_src);
    write(dir, &v0)?;

    // 2026-11-03: same criteria-page grid plus the labour market factor.
    let (modif, modif_src) = load("modifications")?;
    let (wages, wages_src) = load("salaire-eleve")?;
    let mut v1 = v0;
    v1.id = "crs-2026-11-03".into();
    v1.effective_from = date("2026-11-03")?;
    v1.sources.extend([modif_src, wages_src]);
    v1.crs.labour_market = Some(parse_labour_market(&modif)?);
    let list = parse_high_wage(&wages)?;
    if list.effective_from != v1.effective_from {
        return Err(format!("high-wage list effective {:?}", list.effective_from).into());
    }
    v1.crs.high_wage = Some(list);
    write(dir, &v1)?;
    Ok(())
}
