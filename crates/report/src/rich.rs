//! Richer documents for Word only (the lawyer's audit): coloured runs,
//! highlighted blanks, accent bars, card-like grids, banners, framed boxes,
//! bullets, page breaks and a footer with page numbers. Same package as
//! [`crate::to_docx`]; no PDF rendering (the lawyer finalizes the document
//! in Word and saves it as PDF herself). [`to_html`] renders an approximate
//! preview, for checking a layout without Word.

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
    /// Extra letter spacing (twentieths of a point).
    pub spacing: Option<u16>,
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
    pub fn spacing(mut self, twips: u16) -> Self {
        self.spacing = Some(twips);
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
    Right,
    Justify,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Para {
    pub runs: Vec<Run>,
    pub align: Align,
    /// Bullet ("•") with a hanging indent; `2` = second level ("o").
    pub bullet: u8,
    /// Custom bullet mark instead of "•" (e.g. "✓").
    pub mark: Option<(&'static str, &'static str)>,
    /// Space after / before, in twentieths of a point.
    pub after: Option<u16>,
    pub before: Option<u16>,
    pub keep_next: bool,
    /// Accent bar on the left: colour and width (eighths of a point).
    pub bar: Option<(&'static str, u16)>,
    /// Thin rule under the paragraph: colour ("auto" for the text colour).
    pub rule: Option<&'static str>,
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
    pub fn right(mut self) -> Self {
        self.align = Align::Right;
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
    /// A bullet with its own mark and colour.
    pub fn mark(mut self, mark: &'static str, color: &'static str) -> Self {
        self.bullet = 1;
        self.mark = Some((mark, color));
        self
    }
    pub fn after(mut self, twips: u16) -> Self {
        self.after = Some(twips);
        self
    }
    pub fn before(mut self, twips: u16) -> Self {
        self.before = Some(twips);
        self
    }
    pub fn keep_next(mut self) -> Self {
        self.keep_next = true;
        self
    }
    pub fn bar(mut self, color: &'static str, width: u16) -> Self {
        self.bar = Some((color, width));
        self
    }
    pub fn rule(mut self, color: &'static str) -> Self {
        self.rule = Some(color);
        self
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Cell {
    pub paras: Vec<Para>,
    pub fill: Option<&'static str>,
    /// Coloured strip on the left edge (cards): colour and width (eighths of a point).
    pub strip: Option<(&'static str, u16)>,
    /// Content aligned to the top instead of the middle.
    pub top: bool,
}

impl Cell {
    pub fn new(paras: Vec<Para>) -> Self {
        Self {
            paras,
            ..Self::default()
        }
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
    pub fn strip(mut self, color: &'static str, width: u16) -> Self {
        self.strip = Some((color, width));
        self
    }
    pub fn top(mut self) -> Self {
        self.top = true;
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
    /// Modern table: no frame, optional thin lines between rows, and `gap`
    /// twips of white space around cells (cards).
    Grid {
        widths: Vec<u16>,
        header: Option<Vec<Cell>>,
        rows: Vec<Vec<Cell>>,
        lines: Option<&'static str>,
        gap: u16,
    },
    /// Paragraphs inside a thin frame.
    Boxed(Vec<Para>),
    /// A PNG at the full text width; `px` (width, height) sets its height.
    Image {
        png: &'static [u8],
        px: (u32, u32),
    },
    PageBreak,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Doc {
    pub items: Vec<Item>,
    /// Text at the bottom of every page, followed by « page N / M ».
    pub footer: Option<String>,
    /// Body font (default Calibri).
    pub font: Option<&'static str>,
    /// Body text size in half-points (default 20, i.e. 10 pt).
    pub size: Option<u16>,
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
    if let Some(sp) = r.spacing {
        rpr.push_str(&format!("<w:spacing w:val=\"{sp}\"/>"));
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
    if p.bar.is_some() || p.rule.is_some() {
        ppr.push_str("<w:pBdr>");
        if let Some((c, w)) = p.bar {
            ppr.push_str(&format!(
                "<w:left w:val=\"single\" w:sz=\"{w}\" w:space=\"8\" w:color=\"{c}\"/>"
            ));
        }
        if let Some(c) = p.rule {
            ppr.push_str(&format!(
                "<w:bottom w:val=\"single\" w:sz=\"4\" w:space=\"1\" w:color=\"{c}\"/>"
            ));
        }
        ppr.push_str("</w:pBdr>");
    }
    if p.after.is_some() || p.before.is_some() {
        let before = p
            .before
            .map(|b| format!(" w:before=\"{b}\""))
            .unwrap_or_default();
        let after = p
            .after
            .map(|a| format!(" w:after=\"{a}\""))
            .unwrap_or_default();
        ppr.push_str(&format!("<w:spacing{before}{after}/>"));
    }
    match p.bullet {
        0 => {}
        1 => ppr.push_str("<w:ind w:left=\"360\" w:hanging=\"260\"/>"),
        _ => ppr.push_str("<w:ind w:left=\"900\" w:hanging=\"260\"/>"),
    }
    match p.align {
        Align::Left => {}
        Align::Center => ppr.push_str("<w:jc w:val=\"center\"/>"),
        Align::Right => ppr.push_str("<w:jc w:val=\"right\"/>"),
        Align::Justify => ppr.push_str("<w:jc w:val=\"both\"/>"),
    }
    let mut body = String::new();
    if p.bullet > 0 {
        let mark = match (p.mark, p.bullet) {
            (Some((m, c)), _) => Run::new(format!("{m}\t")).color(c).bold(),
            (None, 1) => Run::new("•\t"),
            _ => Run::new("◦\t").color("0F766E").bold(),
        };
        body.push_str(&run_xml(&mark));
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
    let strip = c
        .strip
        .map(|(col, w)| {
            format!(
                "<w:tcBorders><w:left w:val=\"single\" w:sz=\"{w}\" w:space=\"0\" w:color=\"{col}\"/></w:tcBorders>"
            )
        })
        .unwrap_or_default();
    let valign = if c.top { "top" } else { "center" };
    let paras: String = if c.paras.is_empty() {
        "<w:p/>".into()
    } else {
        c.paras.iter().map(|p| para_xml(p, links)).collect()
    };
    format!(
        "<w:tc><w:tcPr><w:tcW w:w=\"{width}\" w:type=\"dxa\"/>{strip}{fill}<w:vAlign w:val=\"{valign}\"/></w:tcPr>{paras}</w:tc>"
    )
}

fn columns(widths: &[u16]) -> Vec<u32> {
    let total: u32 = widths.iter().map(|w| u32::from(*w)).sum::<u32>().max(1);
    widths
        .iter()
        .map(|w| u32::from(*w) * TEXT_WIDTH / total)
        .collect()
}

fn table_xml(
    widths: &[u16],
    header: Option<&Vec<Cell>>,
    rows: &[Vec<Cell>],
    borders: &str,
    gap: u16,
    links: &mut Vec<String>,
) -> String {
    let cols = columns(widths);
    let grid: String = cols
        .iter()
        .map(|w| format!("<w:gridCol w:w=\"{w}\"/>"))
        .collect();
    let spacing = if gap > 0 {
        format!("<w:tblCellSpacing w:w=\"{}\" w:type=\"dxa\"/>", gap / 2)
    } else {
        String::new()
    };
    let pad = if gap > 0 { 140 } else { 90 };
    let mut out = format!(
        "<w:tbl><w:tblPr><w:tblW w:w=\"5000\" w:type=\"pct\"/>{spacing}<w:tblBorders>{borders}</w:tblBorders><w:tblCellMar><w:top w:w=\"{pad}\" w:type=\"dxa\"/><w:left w:w=\"150\" w:type=\"dxa\"/><w:bottom w:w=\"{pad}\" w:type=\"dxa\"/><w:right w:w=\"150\" w:type=\"dxa\"/></w:tblCellMar></w:tblPr><w:tblGrid>{grid}</w:tblGrid>"
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

/// English Metric Units per twip.
const EMU_PER_TWIP: u64 = 635;

/// An inline picture; `n` numbers it (relationship `rIdI{n}`, `media/image{n}.png`).
fn image_xml(n: usize, px: (u32, u32)) -> String {
    let cx = u64::from(TEXT_WIDTH) * EMU_PER_TWIP;
    let cy = cx * u64::from(px.1) / u64::from(px.0.max(1));
    format!(
        "<w:p><w:r><w:drawing><wp:inline distT=\"0\" distB=\"0\" distL=\"0\" distR=\"0\"><wp:extent cx=\"{cx}\" cy=\"{cy}\"/><wp:docPr id=\"{n}\" name=\"Image {n}\"/><wp:cNvGraphicFramePr><a:graphicFrameLocks xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" noChangeAspect=\"1\"/></wp:cNvGraphicFramePr><a:graphic xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\"><a:graphicData uri=\"http://schemas.openxmlformats.org/drawingml/2006/picture\"><pic:pic xmlns:pic=\"http://schemas.openxmlformats.org/drawingml/2006/picture\"><pic:nvPicPr><pic:cNvPr id=\"{n}\" name=\"image{n}.png\"/><pic:cNvPicPr/></pic:nvPicPr><pic:blipFill><a:blip r:embed=\"rIdI{n}\"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill><pic:spPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"{cx}\" cy=\"{cy}\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></pic:spPr></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r></w:p>"
    )
}

fn item_xml(item: &Item, links: &mut Vec<String>, images: &mut Vec<&'static [u8]>) -> String {
    match item {
        Item::Para(p) => para_xml(p, links),
        Item::Banner { lines, fill } => {
            let border = "w:val=\"double\" w:sz=\"12\" w:space=\"0\" w:color=\"1B5E7A\"";
            let borders = format!(
                "<w:top {border}/><w:left {border}/><w:bottom {border}/><w:right {border}/>"
            );
            let cell = Cell::new(
                lines
                    .iter()
                    .map(|r| Para::new(vec![r.clone()]).center())
                    .collect(),
            )
            .fill(fill);
            table_xml(&[1], None, &[vec![cell]], &borders, 0, links)
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
            table_xml(widths, header.as_ref(), rows, &borders, 0, links)
        }
        Item::Grid {
            widths,
            header,
            rows,
            lines,
            gap,
        } => {
            let none = "w:val=\"nil\"";
            let inside = lines.map_or_else(
                || none.to_owned(),
                |c| format!("w:val=\"single\" w:sz=\"4\" w:space=\"0\" w:color=\"{c}\""),
            );
            let borders = format!(
                "<w:top {none}/><w:left {none}/><w:bottom {none}/><w:right {none}/><w:insideH {inside}/><w:insideV {none}/>"
            );
            table_xml(widths, header.as_ref(), rows, &borders, *gap, links)
        }
        Item::Boxed(paras) => {
            let b = "w:val=\"single\" w:sz=\"6\" w:space=\"0\" w:color=\"404040\"";
            let borders = format!("<w:top {b}/><w:left {b}/><w:bottom {b}/><w:right {b}/>");
            table_xml(&[1], None, &[vec![Cell::new(paras.clone())]], &borders, 0, links)
        }
        Item::Image { png, px } => {
            images.push(png);
            image_xml(images.len(), *px)
        }
        Item::PageBreak => "<w:p><w:r><w:br w:type=\"page\"/></w:r></w:p>".into(),
    }
}

fn styles(font: &str, size: u16) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
<w:docDefaults><w:rPrDefault><w:rPr><w:rFonts w:ascii="{font}" w:hAnsi="{font}" w:cs="{font}" w:eastAsia="{font}"/><w:color w:val="1F2937"/><w:sz w:val="{size}"/><w:szCs w:val="{size}"/><w:lang w:val="fr-CA"/></w:rPr></w:rPrDefault>
<w:pPrDefault><w:pPr><w:spacing w:after="80" w:line="276" w:lineRule="auto"/></w:pPr></w:pPrDefault></w:docDefaults>
<w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/></w:style>
<w:style w:type="paragraph" w:styleId="Footer"><w:name w:val="footer"/><w:basedOn w:val="Normal"/><w:pPr><w:spacing w:after="0"/></w:pPr><w:rPr><w:color w:val="6B7280"/><w:sz w:val="16"/></w:rPr></w:style>
<w:style w:type="table" w:default="1" w:styleId="TableNormal"><w:name w:val="Normal Table"/><w:tblPr><w:tblCellMar><w:left w:w="108" w:type="dxa"/><w:right w:w="108" w:type="dxa"/></w:tblCellMar></w:tblPr></w:style>
</w:styles>"#
    )
}

/// Word 2013+ layout (without it Word opens the file in Compatibility Mode).
const SETTINGS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:settings xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:defaultTabStop w:val="708"/><w:compat><w:compatSetting w:name="compatibilityMode" w:uri="http://schemas.microsoft.com/office/word" w:val="15"/></w:compat></w:settings>"#;

/// The footer: a borderless two-column table (text left, « page N / M »
/// right). A right tab stop wraps the page number in Word Online.
fn footer_xml(text: &str) -> String {
    let field = |instr: &str| {
        format!(
            "<w:r><w:fldChar w:fldCharType=\"begin\"/></w:r><w:r><w:instrText xml:space=\"preserve\"> {instr} </w:instrText></w:r><w:r><w:fldChar w:fldCharType=\"separate\"/></w:r><w:r><w:t>1</w:t></w:r><w:r><w:fldChar w:fldCharType=\"end\"/></w:r>"
        )
    };
    let none = "w:val=\"nil\"";
    let line = "w:val=\"single\" w:sz=\"4\" w:space=\"0\" w:color=\"E5E7EB\"";
    let cell = |w: u32, jc: &str, body: String| {
        format!(
            "<w:tc><w:tcPr><w:tcW w:w=\"{w}\" w:type=\"dxa\"/></w:tcPr><w:p><w:pPr><w:pStyle w:val=\"Footer\"/><w:jc w:val=\"{jc}\"/></w:pPr>{body}</w:p></w:tc>"
        )
    };
    let left = cell(
        7638,
        "left",
        format!("<w:r><w:t xml:space=\"preserve\">{}</w:t></w:r>", esc(text)),
    );
    let right = cell(
        2000,
        "right",
        format!(
            "<w:r><w:t xml:space=\"preserve\">page </w:t></w:r>{}<w:r><w:t xml:space=\"preserve\"> / </w:t></w:r>{}",
            field("PAGE"),
            field("NUMPAGES")
        ),
    );
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:ftr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:tbl><w:tblPr><w:tblW w:w="5000" w:type="pct"/><w:tblBorders><w:top {line}/><w:left {none}/><w:bottom {none}/><w:right {none}/><w:insideH {none}/><w:insideV {none}/></w:tblBorders><w:tblCellMar><w:top w:w="80" w:type="dxa"/><w:left w:w="0" w:type="dxa"/><w:right w:w="0" w:type="dxa"/></w:tblCellMar></w:tblPr><w:tblGrid><w:gridCol w:w="7638"/><w:gridCol w:w="2000"/></w:tblGrid><w:tr>{left}{right}</w:tr></w:tbl><w:p><w:pPr><w:pStyle w:val="Footer"/></w:pPr></w:p></w:ftr>"#
    )
}

const CONTENT_TYPES: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
<Default Extension="xml" ContentType="application/xml"/>
<Default Extension="png" ContentType="image/png"/>
<Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
<Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/>
<Override PartName="/word/settings.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml"/>
<Override PartName="/word/footer1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml"/>
</Types>"#;

const RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"#;

/// Renders the document as a Word file.
pub fn to_docx(d: &Doc) -> Vec<u8> {
    let mut links = Vec::new();
    let mut images = Vec::new();
    let body: String = d
        .items
        .iter()
        .map(|i| item_xml(i, &mut links, &mut images))
        .collect();
    let footer_ref = if d.footer.is_some() {
        "<w:footerReference w:type=\"default\" r:id=\"rIdF1\"/>"
    } else {
        ""
    };
    let document = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing"><w:body>{body}<w:sectPr>{footer_ref}<w:pgSz w:w="11906" w:h="16838"/><w:pgMar w:top="1134" w:right="1134" w:bottom="1134" w:left="1134" w:header="567" w:footer="510" w:gutter="0"/></w:sectPr></w:body></w:document>"#
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
    let image_rels: String = (1..=images.len())
        .map(|n| {
            format!(
                "<Relationship Id=\"rIdI{n}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/image\" Target=\"media/image{n}.png\"/>"
            )
        })
        .collect();
    let doc_rels = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/settings" Target="settings.xml"/><Relationship Id="rIdF1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer" Target="footer1.xml"/>{link_rels}{image_rels}
</Relationships>"#
    );
    let names: Vec<String> = (1..=images.len())
        .map(|n| format!("word/media/image{n}.png"))
        .collect();
    let mut parts = vec![
        ("[Content_Types].xml", CONTENT_TYPES.as_bytes().to_vec()),
        ("_rels/.rels", RELS.as_bytes().to_vec()),
        ("word/_rels/document.xml.rels", doc_rels.into_bytes()),
        ("word/document.xml", document.into_bytes()),
        (
            "word/styles.xml",
            styles(d.font.unwrap_or("Calibri"), d.size.unwrap_or(20)).into_bytes(),
        ),
        ("word/settings.xml", SETTINGS.as_bytes().to_vec()),
        (
            "word/footer1.xml",
            footer_xml(d.footer.as_deref().unwrap_or("")).into_bytes(),
        ),
    ];
    parts.extend(
        names
            .iter()
            .map(String::as_str)
            .zip(images.iter().map(|i| i.to_vec())),
    );
    zip(&parts)
}

// ---- HTML preview (approximate) ----------------------------------------------

fn base64(data: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let n = (u32::from(c[0]) << 16)
            | (u32::from(*c.get(1).unwrap_or(&0)) << 8)
            | u32::from(*c.get(2).unwrap_or(&0));
        for i in 0..4 {
            if i <= c.len() {
                out.push(char::from(A[((n >> (18 - 6 * i)) & 63) as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

fn html_esc(s: &str) -> String {
    esc(s).replace('\n', "<br>")
}

fn run_html(r: &Run) -> String {
    let mut css = String::new();
    if r.bold {
        css.push_str("font-weight:700;");
    }
    if r.italic {
        css.push_str("font-style:italic;");
    }
    if r.small_caps {
        css.push_str("font-variant:small-caps;");
    }
    if let Some(c) = r.color {
        css.push_str(&format!("color:#{c};"));
    }
    if let Some(sp) = r.spacing {
        css.push_str(&format!("letter-spacing:{}pt;", f32::from(sp) / 20.0));
    }
    if let Some(sz) = r.size {
        css.push_str(&format!("font-size:{}pt;", f32::from(sz) / 2.0));
    }
    if r.highlight {
        css.push_str("background:#ffff00;");
    }
    if r.link.is_some() {
        css.push_str("text-decoration:underline;");
    }
    format!("<span style=\"{css}\">{}</span>", html_esc(&r.text))
}

fn para_html(p: &Para) -> String {
    let mut css = String::from("margin:0;");
    css.push_str(&format!(
        "margin-bottom:{}pt;",
        f32::from(p.after.unwrap_or(80)) / 20.0
    ));
    if let Some(b) = p.before {
        css.push_str(&format!("margin-top:{}pt;", f32::from(b) / 20.0));
    }
    if let Some((c, w)) = p.bar {
        css.push_str(&format!(
            "border-left:{}pt solid #{c};padding-left:8pt;",
            f32::from(w) / 8.0
        ));
    }
    if let Some(c) = p.rule {
        let c = if c == "auto" {
            "currentColor".into()
        } else {
            format!("#{c}")
        };
        css.push_str(&format!("border-bottom:0.5pt solid {c};"));
    }
    css.push_str(match p.align {
        Align::Left => "",
        Align::Center => "text-align:center;",
        Align::Right => "text-align:right;",
        Align::Justify => "text-align:justify;",
    });
    let mut body = String::new();
    if p.bullet > 0 {
        css.push_str(&format!(
            "padding-left:{}pt;text-indent:-13pt;",
            if p.bullet == 1 { 18 } else { 45 }
        ));
        let mark = match (p.mark, p.bullet) {
            (Some((m, c)), _) => run_html(&Run::new(m).color(c).bold()),
            (None, 1) => "•".into(),
            _ => "o".into(),
        };
        body.push_str(&format!("{mark}&nbsp;&nbsp;"));
    }
    body.extend(p.runs.iter().map(run_html));
    format!("<p style=\"{css}\">{body}</p>")
}

fn table_html(
    widths: &[u16],
    header: Option<&Vec<Cell>>,
    rows: &[Vec<Cell>],
    css: &str,
    gap: u16,
    lines: Option<&str>,
) -> String {
    let total: u32 = widths.iter().map(|w| u32::from(*w)).sum::<u32>().max(1);
    let cols: String = widths
        .iter()
        .map(|w| format!("<col style=\"width:{}%\">", u32::from(*w) * 100 / total))
        .collect();
    let spacing = if gap > 0 {
        format!(
            "border-collapse:separate;border-spacing:0 {}pt;",
            f32::from(gap) / 20.0
        )
    } else {
        "border-collapse:collapse;".into()
    };
    let mut out = format!("<table style=\"width:100%;{spacing}{css}\">{cols}");
    let line = lines
        .map(|c| format!("border-bottom:0.5pt solid #{c};"))
        .unwrap_or_default();
    for (i, row) in header.into_iter().chain(rows.iter()).enumerate() {
        out.push_str("<tr>");
        for c in row {
            let mut s = format!(
                "padding:{}pt 7.5pt;vertical-align:{};",
                if gap > 0 { 7 } else { 4.5 as i32 },
                if c.top { "top" } else { "middle" }
            );
            if let Some(f) = c.fill {
                s.push_str(&format!("background:#{f};"));
            }
            if let Some((col, w)) = c.strip {
                s.push_str(&format!(
                    "border-left:{}pt solid #{col};",
                    f32::from(w) / 8.0
                ));
            }
            if i + 1 < rows.len() + usize::from(header.is_some()) {
                s.push_str(&line);
            }
            let paras: String = c.paras.iter().map(para_html).collect();
            out.push_str(&format!("<td style=\"{s}\">{paras}</td>"));
        }
        out.push_str("</tr>");
    }
    out.push_str("</table><div style=\"height:6pt\"></div>");
    out
}

/// An approximate HTML rendering of the document (A4-wide page, page
/// breaks as separators), to check a layout without Word.
pub fn to_html(d: &Doc) -> String {
    let font = d.font.unwrap_or("Calibri");
    let size = f32::from(d.size.unwrap_or(20)) / 2.0;
    let mut body = String::new();
    for i in &d.items {
        body.push_str(&match i {
            Item::Para(p) => para_html(p),
            Item::Banner { lines, fill } => {
                let paras: Vec<Para> = lines.iter().map(|r| Para::new(vec![r.clone()]).center()).collect();
                table_html(&[1], None, &[vec![Cell::new(paras).fill(fill)]], "border:3px double #1B5E7A;", 0, None)
            }
            Item::Heading { text, color } => format!(
                "<p style=\"margin:14pt 0 6pt;border-bottom:0.75pt solid #1F2937;font-weight:700;font-size:12pt;color:#{color}\">{}</p>",
                html_esc(&text.to_uppercase())
            ),
            Item::Table { widths, header, rows, rule } => table_html(
                widths,
                header.as_ref(),
                rows,
                &format!("border-top:1pt solid #{rule};border-bottom:1pt solid #{rule};"),
                0,
                Some("D9D9D9"),
            ),
            Item::Grid { widths, header, rows, lines, gap } => {
                table_html(widths, header.as_ref(), rows, "", *gap, *lines)
            }
            Item::Boxed(ps) => table_html(&[1], None, &[vec![Cell::new(ps.clone())]], "border:0.75pt solid #404040;", 0, None),
            Item::Image { png, .. } => format!(
                "<p style=\"margin:0 0 4pt\"><img alt=\"\" style=\"width:100%\" src=\"data:image/png;base64,{}\"></p>",
                base64(png)
            ),
            Item::PageBreak => "<hr style=\"border:0;border-top:2px dashed #bbb;margin:24pt -20mm\">".into(),
        });
    }
    let footer = d
        .footer
        .as_deref()
        .map(|f| format!("<p style=\"margin-top:24pt;border-top:0.5pt solid #E5E7EB;padding-top:6pt;color:#6B7280;font-size:8pt\">{} <span style=\"float:right\">page 1 / 1</span></p>", html_esc(f)))
        .unwrap_or_default();
    format!(
        "<!doctype html><html lang=\"fr\"><head><meta charset=\"utf-8\"><style>body{{background:#e5e7eb;margin:0}}.page{{background:#fff;width:170mm;margin:12mm auto;padding:20mm;font-family:'{font}',Calibri,Arial,sans-serif;font-size:{size}pt;line-height:1.38;color:#1F2937;box-shadow:0 1px 4px rgba(0,0,0,.15)}}</style></head><body><div class=\"page\">{body}{footer}</div></body></html>"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Doc {
        Doc {
            footer: Some("Pied de page".into()),
            font: Some("Aptos"),
            size: Some(22),
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
                Item::Grid {
                    widths: vec![1],
                    header: None,
                    rows: vec![vec![Cell::text("carte").strip("0F766E", 24).top()]],
                    lines: Some("E5E7EB"),
                    gap: 160,
                },
                Item::Para(
                    Para::text("Titre")
                        .bar("0F766E", 24)
                        .rule("auto")
                        .before(200),
                ),
                Item::Para(Para::text("fait").mark("✓", "0F766E")),
                Item::Boxed(vec![Para::text("Remarque & <note>")]),
                Item::Para(
                    Para::new(vec![Run::new("lien").link("https://example.com/a?b&c")]).bullet(1),
                ),
                Item::Image {
                    png: b"\x89PNG",
                    px: (2, 1),
                },
                Item::PageBreak,
            ],
        }
    }

    #[test]
    fn renders_every_item_into_a_docx_package() {
        let bytes = to_docx(&sample());
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
            "w:tblCellSpacing w:w=\"80\"",
            "<w:left w:val=\"single\" w:sz=\"24\" w:space=\"0\" w:color=\"0F766E\"/>",
            "compatibilityMode",
            " PAGE ",
            "Pied de page",
            "w:ascii=\"Aptos\"",
            "<wp:extent cx=\"6120130\" cy=\"3060065\"/>",
            "<a:blip r:embed=\"rIdI1\"/>",
            "Target=\"media/image1.png\"",
            "word/media/image1.png",
            "Extension=\"png\"",
            "<w:sz w:val=\"22\"/><w:szCs w:val=\"22\"/><w:lang",
            "<w:bottom w:val=\"single\" w:sz=\"4\" w:space=\"1\" w:color=\"auto\"/>",
        ] {
            assert!(text.contains(part), "{part}");
        }
    }

    #[test]
    fn html_preview_shows_the_same_content() {
        let h = to_html(&sample());
        for part in [
            "CABINET",
            "SYNTHÈSE",
            "background:#ffff00",
            "border-left:3pt solid #0F766E",
            "border-bottom:0.5pt solid currentColor",
            "font-size:11pt",
            "data:image/png;base64,iVBORw==",
            "Pied de page",
            "✓",
        ] {
            assert!(h.contains(part), "{part}");
        }
    }
}
