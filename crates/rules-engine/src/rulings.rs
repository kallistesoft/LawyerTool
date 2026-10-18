//! Lawyer rulings on divergences between official sources
//! (`fixtures/crs/divergences.md`). Applied to a parsed grid before approval;
//! every patched value is cited as `ruling:<id>` so notes can disclose it.

use crate::grid::{Cite, Grid, Points, Range, Row};

fn cite(id: &str, text: &str) -> Cite {
    Cite {
        source: format!("ruling:{id}"),
        table: None,
        row: None,
        text: text.to_owned(),
    }
}

/// Applies every ruling that is relevant to `grid`. Idempotent.
pub fn apply(grid: &mut Grid) {
    d002_second_language_nclc4(grid);
}

/// D-002: the criteria page skips NCLC 4 for the second official language.
fn d002_second_language_nclc4(grid: &mut Grid) {
    let rows = &mut grid.crs.second_language;
    if rows.iter().any(|r| r.key.contains(4)) {
        return;
    }
    let at = rows
        .iter()
        .position(|r| r.key.min > 4)
        .unwrap_or(rows.len());
    rows.insert(
        at,
        Row {
            key: Range {
                min: 4,
                max: Some(4),
            },
            points: Points {
                with_spouse: 0,
                without_spouse: 0,
            },
            cite: cite(
                "D-002",
                "NCLC 4 (deuxième langue officielle) : 0 point, comme le calculateur",
            ),
        },
    );
}
