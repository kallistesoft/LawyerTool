//! Generic page model: the tables of `<main>` in document order, each with
//! the heading that precedes it, plus the flattened running text.

use rules_engine::grid::Cite;
use scraper::{ElementRef, Html, Selector};

use crate::ParseError;

#[derive(Debug)]
pub struct Table {
    pub index: u16,
    pub heading: String,
    /// `<caption>` text, empty if none.
    pub caption: String,
    /// Normalized cell text, header row first.
    pub rows: Vec<Vec<String>>,
}

#[derive(Debug)]
pub struct Page {
    pub source: String,
    pub tables: Vec<Table>,
    pub text: String,
}

/// Collapses whitespace (incl. NBSP), unifies apostrophes, trims.
pub fn norm(s: &str) -> String {
    s.replace(['\u{2019}', '\u{2018}'], "'")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn text_of(el: ElementRef) -> String {
    norm(&el.text().collect::<Vec<_>>().join(" "))
}

fn sel(s: &str) -> Selector {
    // Selectors are compile-time constants in this crate; a bad one is a bug.
    #[allow(clippy::expect_used)]
    Selector::parse(s).expect("valid selector")
}

impl Page {
    pub fn parse(source: &str, html: &str) -> Result<Self, ParseError> {
        let doc = Html::parse_document(html);
        let main = doc.select(&sel("main")).next().ok_or(ParseError::NoMain)?;
        let mut tables = Vec::new();
        let mut heading = String::new();
        for el in main.select(&sel("h2, h3, h4, table")) {
            if el.value().name() == "table" {
                let rows = el
                    .select(&sel("tr"))
                    .map(|tr| tr.select(&sel("th, td")).map(text_of).collect())
                    .collect();
                let index = u16::try_from(tables.len()).map_err(|_| ParseError::TooManyTables)?;
                let caption = el
                    .select(&sel("caption"))
                    .next()
                    .map(text_of)
                    .unwrap_or_default();
                tables.push(Table {
                    index,
                    heading: heading.clone(),
                    caption,
                    rows,
                });
            } else {
                heading = text_of(el);
            }
        }
        Ok(Self {
            source: source.to_owned(),
            tables,
            text: text_of(main),
        })
    }

    /// The single table matching `pred`; zero or several matches is an error.
    pub fn table(
        &self,
        what: &'static str,
        pred: impl Fn(&Table) -> bool,
    ) -> Result<&Table, ParseError> {
        let found: Vec<&Table> = self.tables.iter().filter(|t| pred(t)).collect();
        match found[..] {
            [t] => Ok(t),
            [] => Err(ParseError::TableNotFound(what)),
            _ => Err(ParseError::AmbiguousTable {
                what,
                count: found.len(),
            }),
        }
    }

    pub fn text_cite(&self, text: &str) -> Cite {
        Cite {
            source: self.source.clone(),
            table: None,
            row: None,
            text: text.to_owned(),
        }
    }
}

impl Table {
    pub fn header(&self) -> &[String] {
        self.rows.first().map(Vec::as_slice).unwrap_or(&[])
    }

    pub fn h(&self, col: usize) -> &str {
        self.header().get(col).map(String::as_str).unwrap_or("")
    }

    /// Data rows (after the header) with their row index.
    pub fn body(&self) -> impl Iterator<Item = (u16, &[String])> {
        self.rows
            .iter()
            .enumerate()
            .skip(1)
            .map(|(i, r)| (u16::try_from(i).unwrap_or(u16::MAX), r.as_slice()))
    }

    pub fn cite(&self, source: &str, row: u16) -> Cite {
        let text = self
            .rows
            .get(usize::from(row))
            .map(|r| r.join(" | "))
            .unwrap_or_default();
        Cite {
            source: source.to_owned(),
            table: Some(self.index),
            row: Some(row),
            text,
        }
    }

    /// Checks every row has exactly `n` cells.
    pub fn expect_cols(&self, what: &'static str, n: usize) -> Result<(), ParseError> {
        match self.rows.iter().position(|r| r.len() != n) {
            None => Ok(()),
            Some(row) => Err(ParseError::Shape {
                what,
                table: self.index,
                row,
                expected: n,
            }),
        }
    }
}
