//! PDF 1.4 writer: A4 pages, Helvetica / Helvetica-Bold (standard fonts,
//! WinAnsi encoding, so French accents work without embedding a font),
//! word wrapping, simple tables, page footer with "page n / N".

use crate::fonts::{text_width, winansi};
use crate::{Block, Report};

const PAGE_W: f32 = 595.0;
const PAGE_H: f32 = 842.0;
const MARGIN: f32 = 56.0;
const BOTTOM: f32 = 64.0; // keeps room for the footer
const CONTENT_W: f32 = PAGE_W - 2.0 * MARGIN;
const CELL_PAD: f32 = 3.0;

#[derive(Clone, Copy)]
struct Style {
    size: f32,
    bold: bool,
    grey: bool,
}

const TITLE: Style = Style {
    size: 16.0,
    bold: true,
    grey: false,
};
const HEADING: Style = Style {
    size: 12.0,
    bold: true,
    grey: false,
};
const BODY: Style = Style {
    size: 10.0,
    bold: false,
    grey: false,
};
const BODY_BOLD: Style = Style {
    size: 10.0,
    bold: true,
    grey: false,
};
const SMALL: Style = Style {
    size: 7.5,
    bold: false,
    grey: true,
};
const CELL: Style = Style {
    size: 8.5,
    bold: false,
    grey: false,
};
const CELL_HEAD: Style = Style {
    size: 8.5,
    bold: true,
    grey: false,
};

fn line_height(s: Style) -> f32 {
    s.size * 1.3
}

/// Greedy word wrap to `width` points; long words are cut by characters.
fn wrap(text: &str, s: Style, width: f32) -> Vec<String> {
    let mut lines = Vec::new();
    for para in text.split('\n') {
        let mut line = String::new();
        for word in para.split(' ') {
            let candidate = if line.is_empty() {
                word.to_owned()
            } else {
                format!("{line} {word}")
            };
            if text_width(&candidate, s.size, s.bold) <= width {
                line = candidate;
                continue;
            }
            if !line.is_empty() {
                lines.push(std::mem::take(&mut line));
            }
            // A single word wider than the line: cut it.
            let mut chunk = String::new();
            for c in word.chars() {
                let next = format!("{chunk}{c}");
                if text_width(&next, s.size, s.bold) > width && !chunk.is_empty() {
                    lines.push(std::mem::take(&mut chunk));
                    chunk.push(c);
                } else {
                    chunk = next;
                }
            }
            line = chunk;
        }
        lines.push(line);
    }
    lines
}

fn pdf_string(text: &str) -> String {
    let mut out = String::from("(");
    for c in text.chars() {
        match winansi(c) {
            b'(' => out.push_str("\\("),
            b')' => out.push_str("\\)"),
            b'\\' => out.push_str("\\\\"),
            b @ 32..=126 => out.push(char::from(b)),
            b => out.push_str(&format!("\\{b:03o}")),
        }
    }
    out.push(')');
    out
}

struct Writer {
    pages: Vec<String>,
    cur: String,
    y: f32,
}

impl Writer {
    fn new() -> Self {
        Self {
            pages: Vec::new(),
            cur: String::new(),
            y: PAGE_H - MARGIN,
        }
    }

    fn new_page(&mut self) {
        self.pages.push(std::mem::take(&mut self.cur));
        self.y = PAGE_H - MARGIN;
    }

    fn ensure(&mut self, height: f32) {
        if self.y - height < BOTTOM {
            self.new_page();
        }
    }

    fn text_at(&mut self, x: f32, y: f32, s: Style, text: &str) {
        let font = if s.bold { "F2" } else { "F1" };
        let grey = if s.grey { "0.35 g" } else { "0 g" };
        self.cur.push_str(&format!(
            "BT {grey} /{font} {:.1} Tf {x:.2} {y:.2} Td {} Tj ET\n",
            s.size,
            pdf_string(text)
        ));
    }

    fn rule(&mut self, x1: f32, x2: f32, y: f32, grey: f32) {
        self.cur.push_str(&format!(
            "{grey:.2} G 0.4 w {x1:.2} {y:.2} m {x2:.2} {y:.2} l S\n"
        ));
    }

    fn paragraph(&mut self, text: &str, s: Style, indent: f32, gap_after: f32) {
        for line in wrap(text, s, CONTENT_W - indent) {
            self.ensure(line_height(s));
            self.y -= line_height(s);
            self.text_at(MARGIN + indent, self.y + s.size * 0.25, s, &line);
        }
        self.y -= gap_after;
    }

    fn notice(&mut self, text: &str) {
        let lines = wrap(text, BODY_BOLD, CONTENT_W - 12.0);
        let h = lines.len() as f32 * line_height(BODY_BOLD) + 8.0;
        self.ensure(h);
        let top = self.y;
        self.cur.push_str(&format!(
            "0.85 0.45 0 RG 2 w {:.2} {:.2} m {:.2} {:.2} l S\n",
            MARGIN,
            top - 2.0,
            MARGIN,
            top - h + 2.0
        ));
        self.y -= 4.0;
        for line in lines {
            self.y -= line_height(BODY_BOLD);
            self.text_at(MARGIN + 10.0, self.y + 2.5, BODY_BOLD, &line);
        }
        self.y -= 10.0;
    }

    fn table(&mut self, widths: &[u16], rows: &[Vec<String>]) {
        let total: f32 = widths.iter().map(|w| f32::from(*w)).sum::<f32>().max(1.0);
        let cols: Vec<f32> = widths
            .iter()
            .map(|w| f32::from(*w) / total * CONTENT_W)
            .collect();
        for (i, row) in rows.iter().enumerate() {
            let style = if i == 0 { CELL_HEAD } else { CELL };
            let cells: Vec<Vec<String>> = cols
                .iter()
                .enumerate()
                .map(|(c, w)| {
                    wrap(
                        row.get(c).map(String::as_str).unwrap_or(""),
                        style,
                        w - 2.0 * CELL_PAD,
                    )
                })
                .collect();
            let n = cells.iter().map(Vec::len).max().unwrap_or(1).max(1);
            let h = n as f32 * line_height(style) + 2.0 * CELL_PAD;
            self.ensure(h);
            let top = self.y;
            let mut x = MARGIN;
            for (c, lines) in cells.iter().enumerate() {
                let mut y = top - CELL_PAD;
                for l in lines {
                    y -= line_height(style);
                    self.text_at(x + CELL_PAD, y + style.size * 0.25, style, l);
                }
                x += cols[c];
            }
            self.y -= h;
            self.rule(
                MARGIN,
                MARGIN + CONTENT_W,
                self.y,
                if i == 0 { 0.2 } else { 0.75 },
            );
        }
        self.y -= 10.0;
    }

    fn finish(mut self, footer: &str) -> Vec<String> {
        if !self.cur.is_empty() || self.pages.is_empty() {
            self.pages.push(std::mem::take(&mut self.cur));
        }
        let n = self.pages.len();
        self.pages
            .into_iter()
            .enumerate()
            .map(|(i, mut content)| {
                let page = format!("{footer} · page {} / {n}", i + 1);
                let font = SMALL;
                content.push_str(&format!(
                    "BT 0.35 g /F1 {:.1} Tf {:.2} {:.2} Td {} Tj ET\n",
                    font.size,
                    MARGIN,
                    BOTTOM - 28.0,
                    pdf_string(&page)
                ));
                content
            })
            .collect()
    }
}

/// Renders the report as a PDF document.
pub fn to_pdf(r: &Report) -> Vec<u8> {
    let mut w = Writer::new();
    w.paragraph(&r.title, TITLE, 0.0, 8.0);
    for b in &r.blocks {
        match b {
            Block::Title(t) => w.paragraph(t, TITLE, 0.0, 8.0),
            Block::Heading(t) => {
                w.y -= 6.0;
                w.ensure(line_height(HEADING) + 3.0 * line_height(BODY));
                w.paragraph(t, HEADING, 0.0, 4.0);
            }
            Block::Paragraph(t) => w.paragraph(t, BODY, 0.0, 6.0),
            Block::Small(t) => w.paragraph(t, SMALL, 0.0, 4.0),
            Block::Notice(t) => w.notice(t),
            Block::Fields(fields) => {
                for (k, v) in fields {
                    w.paragraph(&format!("{k} : {v}"), BODY, 0.0, 0.0);
                }
                w.y -= 6.0;
            }
            Block::Table { widths, rows } => w.table(widths, rows),
        }
    }
    let pages = w.finish(&r.footer);

    // Objects: 1 catalog, 2 pages, 3 F1, 4 F2, then (page, content) pairs, then info.
    let mut objects: Vec<String> = Vec::new();
    let page_ids: Vec<usize> = (0..pages.len()).map(|i| 5 + 2 * i).collect();
    objects.push("<< /Type /Catalog /Pages 2 0 R >>".into());
    objects.push(format!(
        "<< /Type /Pages /Count {} /Kids [{}] >>",
        pages.len(),
        page_ids
            .iter()
            .map(|id| format!("{id} 0 R"))
            .collect::<Vec<_>>()
            .join(" ")
    ));
    objects.push(
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>".into(),
    );
    objects.push(
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold /Encoding /WinAnsiEncoding >>"
            .into(),
    );
    for (i, content) in pages.iter().enumerate() {
        objects.push(format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {PAGE_W} {PAGE_H}] /Resources << /Font << /F1 3 0 R /F2 4 0 R >> >> /Contents {} 0 R >>",
            page_ids[i] + 1
        ));
        objects.push(format!(
            "<< /Length {} >>\nstream\n{content}endstream",
            content.len()
        ));
    }
    objects.push(format!(
        "<< /Title {} /Producer (LawyerTool) >>",
        pdf_string(&r.title)
    ));
    let info_id = objects.len();

    let mut out = b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n".to_vec();
    let mut offsets = Vec::with_capacity(objects.len());
    for (i, obj) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n{obj}\nendobj\n", i + 1).as_bytes());
    }
    let xref = out.len();
    out.extend_from_slice(
        format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes(),
    );
    for o in offsets {
        out.extend_from_slice(format!("{o:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R /Info {info_id} 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            objects.len() + 1
        )
        .as_bytes(),
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps_within_width() {
        let text =
            "Le demandeur satisfait à l’exigence d’expérience de travail qualifié au Canada.";
        for line in wrap(text, BODY, 150.0) {
            assert!(text_width(&line, BODY.size, false) <= 150.0, "{line}");
        }
        assert_eq!(wrap("", BODY, 100.0), vec![String::new()]);
    }

    #[test]
    fn strings_escape_and_encode() {
        assert_eq!(pdf_string("a(b)c\\"), "(a\\(b\\)c\\\\)");
        assert_eq!(pdf_string("é’"), "(\\351\\222)");
    }
}
