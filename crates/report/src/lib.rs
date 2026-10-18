//! A small document model for assessment reports and notes, rendered to PDF
//! and Word (.docx) by this crate alone: no Office, browser or PDF library
//! needed on the user's machine, and no file or network access (the caller
//! writes the bytes where the user chose).

mod docx;
mod fonts;
mod pdf;
mod zip;

pub use docx::to_docx;
pub use pdf::to_pdf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Block {
    Title(String),
    Heading(String),
    Paragraph(String),
    /// Smaller, grey text (citations, notes).
    Small(String),
    /// Label / value pairs, one per line.
    Fields(Vec<(String, String)>),
    /// `widths` are relative column weights; the first row is the header.
    Table {
        widths: Vec<u16>,
        rows: Vec<Vec<String>>,
    },
    /// Highlighted paragraph (warnings, mandatory review).
    Notice(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub title: String,
    pub blocks: Vec<Block>,
    /// Printed at the bottom of every PDF page (and at the end of the .docx).
    pub footer: String,
}
