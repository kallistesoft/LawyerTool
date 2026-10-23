//! Richer documents for Word only (the lawyer's audit): coloured runs,
//! highlighted blanks, shaded tables, banners, framed boxes, bullets and page
//! breaks. Same package as [`crate::to_docx`]; no PDF rendering (the lawyer
//! finalizes the document in Word and saves it as PDF herself).

use crate::docx::esc;
use crate::zip::zip;

/// A span of text with its formatting. Colours are hex RGB ("1F6B1F").
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Run {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub color: Option<&'static str>,
    /// Yellow highlight: a blank or a value the lawyer must check.
    pub highlight: bool,
    /// Size in half-points.
    pub size: Option<u16>,
    pub small_caps: bool,
    /// Hyperlink target.
    pub link: Option<String>,
}

impl Run {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            ..Self::default()
        }
    }
    pub fn bold(mut self) -> Self {
        self.bold = true;
        self
    }
    pub fn italic(mut self) -> Self {
        self.italic = true;
        self
    }
    pub fn color(mut self, c: &'static str) -> Self {
        self.color = Some(c);
        self
    }
    pub fn highlight(mut self) -> Self {
        self.highlight = true;
        self
    }
    pub fn size(mut self, half_points: u16) -> Self {
        self.size = Some(half_points);
        self
    }
    pub fn small_caps(mut self) -> Self {
        self.small_caps = true;
        self
    }
    pub fn link(mut self, url: impl Into<String>) -> Self {
        self.link = Some(url.into());
        self
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Align {
    #[default]
    Left,
    Center,
    Justify,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Para {
    pub runs: Vec<Run>,
    pub align: Align,
    /// Bullet ("•") with a hanging indent; `2` = second level ("o").
    pub bullet: u8,
    /// Space after, in twentieths of a point.
    pub after: Option<u16>,
    pub keep_next: bool,
}

impl Para {
    pub fn new(runs: Vec<Run>) -> Self {
        Self {
            runs,
            ..Self::default()
        }
    }
    pub fn text(t: impl Into<String>) -> Self {
        Self::new(vec![Run::new(t)])
    }
    pub fn center(mut self) -> Self {
        self.align = Align::Center;
        self
    }
    pub fn justify(mut self) -> Self {
        self.align = Align::Justify;
        self
    }
    pub fn bullet(mut self, level: u8) -> Self {
        self.bullet = level;
        self
    }
    pub fn after(mut self, twips: u16) -> Self {
        self.after = Some(twips);
        self
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Cell {
    pub paras: Vec<Para>,
    pub fill: Option<&'static str>,
}

impl Cell {
    pub fn new(paras: Vec<Para>) -> Self {
        Self { paras, fill: None }
    }
    pub fn runs(runs: Vec<Run>) -> Self {
        Self::new(vec![Para::new(runs)])
    }
    pub fn text(t: impl Into<String>) -> Self {
        Self::runs(vec![Run::new(t)])
    }
    pub fn fill(mut self, c: &'static str) -> Self {
        self.fill = Some(c);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Item {
    Para(Para),
    /// Centred title box with a double border (dark fill, coloured text).
    Banner {
        lines: Vec<Run>,
        fill: &'static str,
    },
    /// Section title in capitals, coloured, with a rule under it.
    Heading {
        text: String,
        color: &'static str,
    },
    /// `widths` are relative; the header row (if any) repeats on each page.
    Table {
        widths: Vec<u16>,
        header: Option<Vec<Cell>>,
        rows: Vec<Vec<Cell>>,
        /// Colour of the table's top and bottom rules.
        rule: &'static str,
    },
    /// Paragraphs inside a thin frame.
    Boxed(Vec<Para>),
    PageBreak,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Doc {
    pub items: Vec<Item>,
}

/// Page text width at A4 with 2 cm margins (twips).
const TEXT_WIDTH: u32 = 9638;

fn run_xml(r: &Run) -> String {
    let mut rpr = String::new();
    if r.bold {
        rpr.push_str("<w:b/>");
    }
    if r.italic {
        rpr.push_str("<w:i/>");
    }
    if r.small_caps {
        rpr.push_str("<w:smallCaps/>");
    }
    if let Some(c) = r.color {
        rpr.push_str(&format!("<w:color w:val=\"{c}\"/>"));
    }
    if let Some(sz) = r.size {
        rpr.push_str(&format!("<w:sz w:val=\"{sz}\"/><w:szCs w:val=\"{sz}\"/>"));
    }
    if r.highlight {
        rpr.push_str("<w:highlight w:val=\"yellow\"/>");
    }
    if r.link.is_some() {
        rpr.push_str("<w:u w:val=\"single\"/>");
    }
    let rpr = if rpr.is_empty() {
        String::new()
    } else {
        format!("<w:rPr>{rpr}</w:rPr>")
    };
    let lines: Vec<String> = r
        .text
        .split('\n')
        .map(|l| format!("<w:t xml:space=\"preserve\">{}</w:t>", esc(l)))
        .collect();
    format!("<w:r>{rpr}{}</w:r>", lines.join("<w:br/>"))
}

fn para_xml(p: &Para, links: &mut Vec<String>) -> String {
    let mut ppr = String::new();
    if p.keep_next {
        ppr.push_str("<w:keepNext/>");
    }
    if let Some(a) = p.after {
        ppr.push_str(&format!("<w:spacing w:after=\"{a}\"/>"));
    }
    match p.bullet {
        0 => {}
        1 => ppr.push_str("<w:ind w:left=\"360\" w:hanging=\"240\"/>"),
        _ => ppr.push_str("<w:ind w:left=\"900\" w:hanging=\"240\"/>"),
    }
    match p.align {
        Align::Left => {}
        Align::Center => ppr.push_str("<w:jc w:val=\"center\"/>"),
        Align::Justify => ppr.push_str("<w:jc w:val=\"both\"/>"),
    }
    let mut body = String::new();
    if p.bullet > 0 {
        let mark = if p.bullet == 1 { "•\t" } else { "o\t" };
        body.push_str(&run_xml(&Run::new(mark)));
    }
    for r in &p.runs {
        match &r.link {
            Some(url) => {
                links.push(url.clone());
                let id = format!("rIdL{}", links.len());
                body.push_str(&format!(
                    "<w:hyperlink r:id=\"{id}\">{}</w:hyperlink>",
                    run_xml(r)
                ));
            }
            None => body.push_str(&run_xml(r)),
        }
    }
    let ppr = if ppr.is_empty() {
        String::new()
    } else {
        format!("<w:pPr>{ppr}</w:pPr>")
    };
    format!("<w:p>{ppr}{body}</w:p>")
}

fn cell_xml(c: &Cell, width: u32, links: &mut Vec<String>) -> String {
    let fill = c
        .fill
        .map(|f| format!("<w:shd w:val=\"clear\" w:color=\"auto\" w:fill=\"{f}\"/>"))
        .unwrap_or_default();
    let paras: String = if c.paras.is_empty() {
        "<w:p/>".into()
    } else {
        c.paras.iter().map(|p| para_xml(p, links)).collect()
    };
    format!(
        "<w:tc><w:tcPr><w:tcW w:w=\"{width}\" w:type=\"dxa\"/>{fill}<w:vAlign w:val=\"center\"/></w:tcPr>{paras}</w:tc>"
    )
}

fn table_xml(
    widths: &[u16],
    header: Option<&Vec<Cell>>,
    rows: &[Vec<Cell>],
    borders: &str,
    links: &mut Vec<String>,
) -> String {
    let total: u32 = widths.iter().map(|w| u32::from(*w)).sum::<u32>().max(1);
    let cols: Vec<u32> = widths
        .iter()
        .map(|w| u32::from(*w) * TEXT_WIDTH / total)
        .collect();
    let grid: String = cols
        .iter()
        .map(|w| format!("<w:gridCol w:w=\"{w}\"/>"))
        .collect();
    let mut out = format!(
        "<w:tbl><w:tblPr><w:tblW w:w=\"5000\" w:type=\"pct\"/><w:tblBorders>{borders}</w:tblBorders><w:tblCellMar><w:top w:w=\"60\" w:type=\"dxa\"/><w:left w:w=\"100\" w:type=\"dxa\"/><w:bottom w:w=\"60\" w:type=\"dxa\"/><w:right w:w=\"100\" w:type=\"dxa\"/></w:tblCellMar></w:tblPr><w:tblGrid>{grid}</w:tblGrid>"
    );
    let mut row_xml = |row: &[Cell], header: bool| {
        out.push_str(if header {
            "<w:tr><w:trPr><w:tblHeader/><w:cantSplit/></w:trPr>"
        } else {
            "<w:tr><w:trPr><w:cantSplit/></w:trPr>"
        });
        for (i, w) in cols.iter().enumerate() {
            let empty = Cell::default();
            out.push_str(&cell_xml(row.get(i).unwrap_or(&empty), *w, links));
        }
        out.push_str("</w:tr>");
    };
    if let Some(h) = header {
        row_xml(h, true);
    }
    for r in rows {
        row_xml(r, false);
    }
    out.push_str("</w:tbl><w:p><w:pPr><w:spacing w:after=\"120\"/></w:pPr></w:p>");
    out
}

fn item_xml(item: &Item, links: &mut Vec<String>) -> String {
    match item {
        Item::Para(p) => para_xml(p, links),
        Item::Banner { lines, fill } => {
            let border = "w:val=\"double\" w:sz=\"12\" w:space=\"0\" w:color=\"1B5E7A\"";
            let borders = format!(
                "<w:top {border}/><w:left {border}/><w:bottom {border}/><w:right {border}/>"
            );
            let cell = Cell::new(lines.iter().map(|r| Para::new(vec![r.clone()]).center()).collect())
                .fill(fill);
            table_xml(&[1], None, &[vec![cell]], &borders, links)
        }
        Item::Heading { text, color } => para_xml(
            &Para {
                runs: vec![Run::new(text.to_uppercase()).bold().color(color).size(24)],
                keep_next: true,
                after: Some(120),
                ..Para::default()
            },
            links,
        )
        .replacen(
            "<w:pPr>",
            "<w:pPr><w:pBdr><w:bottom w:val=\"single\" w:sz=\"6\" w:space=\"1\" w:color=\"1F2937\"/></w:pBdr><w:spacing w:before=\"280\"/>",
            1,
        ),
        Item::Table {
            widths,
            header,
            rows,
            rule,
        } => {
            let edge = format!("w:val=\"single\" w:sz=\"8\" w:space=\"0\" w:color=\"{rule}\"");
            let inside = "w:val=\"single\" w:sz=\"4\" w:space=\"0\" w:color=\"D9D9D9\"";
            let borders = format!("<w:top {edge}/><w:bottom {edge}/><w:insideH {inside}/>");
            table_xml(widths, header.as_ref(), rows, &borders, links)
        }
        Item::Boxed(paras) => {
            let b = "w:val=\"single\" w:sz=\"6\" w:space=\"0\" w:color=\"404040\"";
            let borders = format!("<w:top {b}/><w:left {b}/><w:bottom {b}/><w:right {b}/>");
            table_xml(&[1], None, &[vec![Cell::new(paras.clone())]], &borders, links)
        }
        Item::PageBreak => "<w:p><w:r><w:br w:type=\"page\"/></w:r></w:p>".into(),
    }
}

const STYLES: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
<w:docDefaults><w:rPrDefault><w:rPr><w:rFonts w:ascii="Calibri" w:hAnsi="Calibri" w:cs="Calibri" w:eastAsia="Calibri"/><w:sz w:val="21"/><w:szCs w:val="21"/><w:lang w:val="fr-CA"/></w:rPr></w:rPrDefault>
<w:pPrDefault><w:pPr><w:spacing w:after="80" w:line="264" w:lineRule="auto"/></w:pPr></w:pPrDefault></w:docDefaults>
<w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/></w:style>
<w:style w:type="table" w:default="1" w:styleId="TableNormal"><w:name w:val="Normal Table"/><w:tblPr><w:tblCellMar><w:left w:w="108" w:type="dxa"/><w:right w:w="108" w:type="dxa"/></w:tblCellMar></w:tblPr></w:style>
</w:styles>"#;

const CONTENT_TYPES: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
<Default Extension="xml" ContentType="application/xml"/>
<Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
<Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/>
</Types>"#;

const RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"#;

/// Renders the document as a Word file.
pub fn to_docx(d: &Doc) -> Vec<u8> {
    let mut links = Vec::new();
    let body: String = d.items.iter().map(|i| item_xml(i, &mut links)).collect();
    let document = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><w:body>{body}<w:sectPr><w:pgSz w:w="11906" w:h="16838"/><w:pgMar w:top="1134" w:right="1134" w:bottom="1134" w:left="1134" w:header="567" w:footer="567" w:gutter="0"/></w:sectPr></w:body></w:document>"#
    );
    let link_rels: String = links
        .iter()
        .enumerate()
        .map(|(i, url)| {
            format!(
                "<Relationship Id=\"rIdL{}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink\" Target=\"{}\" TargetMode=\"External\"/>",
                i + 1,
                esc(url)
            )
        })
        .collect();
    let doc_rels = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>{link_rels}
</Relationships>"#
    );
    zip(&[
        ("[Content_Types].xml", CONTENT_TYPES.as_bytes().to_vec()),
        ("_rels/.rels", RELS.as_bytes().to_vec()),
        ("word/_rels/document.xml.rels", doc_rels.into_bytes()),
        ("word/document.xml", document.into_bytes()),
        ("word/styles.xml", STYLES.as_bytes().to_vec()),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_every_item_into_a_docx_package() {
        let d = Doc {
            items: vec![
                Item::Banner {
                    lines: vec![Run::new("CABINET").bold().color("FFD000")],
                    fill: "0B3142",
                },
                Item::Heading {
                    text: "Synthèse".into(),
                    color: "1F2937",
                },
                Item::Table {
                    widths: vec![1, 2],
                    header: Some(vec![Cell::text("A").fill("0B3142"), Cell::text("B")]),
                    rows: vec![vec![
                        Cell::runs(vec![Run::new("●").color("2E9E2E")]),
                        Cell::runs(vec![Run::new("[CNP à valider]").highlight()]),
                    ]],
                    rule: "1F2937",
                },
                Item::Boxed(vec![Para::text("Remarque & <note>")]),
                Item::Para(
                    Para::new(vec![Run::new("lien").link("https://example.com/a?b&c")]).bullet(1),
                ),
                Item::PageBreak,
            ],
        };
        let bytes = to_docx(&d);
        assert!(bytes.starts_with(b"PK"));
        let text = String::from_utf8_lossy(&bytes);
        for part in [
            "w:fill=\"0B3142\"",
            "w:highlight w:val=\"yellow\"",
            "SYNTHÈSE",
            "Remarque &amp; &lt;note&gt;",
            "w:br w:type=\"page\"",
            "Target=\"https://example.com/a?b&amp;c\"",
            "<w:hyperlink r:id=\"rIdL1\">",
        ] {
            assert!(text.contains(part), "{part}");
        }
    }
}
