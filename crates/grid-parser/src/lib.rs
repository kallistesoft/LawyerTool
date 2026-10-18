//! Deterministic parser: official IRCC HTML tables → rules grid.
//!
//! Anchors on headings, header cells and row labels. Any unexpected table,
//! row, label or number is an error: the parser never guesses, and a failure
//! keeps the last approved grid in force.

mod crs;
mod labour;
mod page;

pub use crs::parse_crs;
pub use labour::{parse_high_wage, parse_labour_market};
pub use page::{Page, norm};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ParseError {
    #[error("page has no <main> element")]
    NoMain,
    #[error("page has too many tables")]
    TooManyTables,
    #[error("table not found: {0}")]
    TableNotFound(&'static str),
    #[error("{count} tables match {what}, expected exactly one")]
    AmbiguousTable { what: &'static str, count: usize },
    #[error("{what} (table {table}): row {row} does not have {expected} cells")]
    Shape {
        what: &'static str,
        table: u16,
        row: usize,
        expected: usize,
    },
    #[error("{what} (table {table}): unexpected row {row}: {text:?}")]
    UnexpectedRow {
        what: &'static str,
        table: u16,
        row: u16,
        text: String,
    },
    #[error("{what}: expected row {label:?} not found")]
    MissingRow {
        what: &'static str,
        label: &'static str,
    },
    #[error("{what} (table {table}): row {row}: not a number: {text:?}")]
    BadNumber {
        what: &'static str,
        table: u16,
        row: u16,
        text: String,
    },
    #[error("{what} (table {table}): row {row}: unrecognized label {text:?}")]
    BadLabel {
        what: &'static str,
        table: u16,
        row: u16,
        text: String,
    },
    #[error("running text not found: {0}")]
    TextNotFound(&'static str),
    #[error("{what}: values disagree: {detail}")]
    Inconsistent { what: &'static str, detail: String },
}
