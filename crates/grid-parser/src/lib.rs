//! Deterministic parser: official IRCC HTML tables → rules grid.
//!
//! Anchors on headings, header cells and row labels. Any unexpected table,
//! row, label or number is an error: the parser never guesses, and a failure
//! keeps the last approved grid in force.

mod crs;
mod equivalency;
mod labour;
mod page;
mod programs;

pub use crs::parse_crs;
pub use equivalency::parse_equivalency;
pub use labour::{parse_high_wage, parse_labour_market};
pub use page::{Page, norm};
pub use programs::{ProgramPages, parse_programs};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ParseError {
    #[error("la page n’a pas d’élément <main>")]
    NoMain,
    #[error("la page contient trop de tableaux")]
    TooManyTables,
    #[error("tableau introuvable : {0}")]
    TableNotFound(&'static str),
    #[error("{count} tableaux correspondent à « {what} », un seul attendu")]
    AmbiguousTable { what: &'static str, count: usize },
    #[error("{what} (tableau {table}) : la ligne {row} n’a pas {expected} cellules")]
    Shape {
        what: &'static str,
        table: u16,
        row: usize,
        expected: usize,
    },
    #[error("{what} (tableau {table}) : ligne {row} inattendue : {text:?}")]
    UnexpectedRow {
        what: &'static str,
        table: u16,
        row: u16,
        text: String,
    },
    #[error("{what} : ligne attendue {label:?} introuvable")]
    MissingRow {
        what: &'static str,
        label: &'static str,
    },
    #[error("{what} (tableau {table}) : ligne {row} : nombre invalide : {text:?}")]
    BadNumber {
        what: &'static str,
        table: u16,
        row: u16,
        text: String,
    },
    #[error("{what} (tableau {table}) : ligne {row} : libellé non reconnu {text:?}")]
    BadLabel {
        what: &'static str,
        table: u16,
        row: u16,
        text: String,
    },
    #[error("texte attendu introuvable : {0}")]
    TextNotFound(&'static str),
    #[error("{what} : valeurs contradictoires : {detail}")]
    Inconsistent { what: &'static str, detail: String },
}
