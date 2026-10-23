//! Word (.docx) writer: minimal Office Open XML package (document, styles,
//! content types, relationships) in a stored ZIP.

use crate::zip::zip;
use crate::{Block, Report};

pub(crate) fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            // XML 1.0 forbids most control characters.
            c if (c as u32) < 0x20 && !matches!(c, '\t' | '\n' | '\r') => {}
            c => out.push(c),
        }
    }
    out
}

/// One paragraph; `\n` becomes a line break within it.
fn para(
    text: &str,
    style: Option<&str>,
    bold: bool,
    size_half_pt: Option<u16>,
    grey: bool,
) -> String {
    let ppr = style
        .map(|s| format!("<w:pPr><w:pStyle w:val=\"{s}\"/></w:pPr>"))
        .unwrap_or_default();
    let mut rpr = String::new();
    if bold {
        rpr.push_str("<w:b/>");
    }
    if grey {
        rpr.push_str("<w:color w:val=\"595959\"/>");
    }
    if let Some(sz) = size_half_pt {
        rpr.push_str(&format!("<w:sz w:val=\"{sz}\"/>"));
    }
    let rpr = if rpr.is_empty() {
        String::new()
    } else {
        format!("<w:rPr>{rpr}</w:rPr>")
    };
    let runs: Vec<String> = text
        .split('\n')
        .map(|line| {
            format!(
                "<w:r>{rpr}<w:t xml:space=\"preserve\">{}</w:t></w:r>",
                esc(line)
            )
        })
        .collect();
    format!("<w:p>{ppr}{}</w:p>", runs.join("<w:r><w:br/></w:r>"))
}

fn table(widths: &[u16], rows: &[Vec<String>]) -> String {
    // Page text width at A4 with 2 cm margins ≈ 9638 twips.
    let total: u32 = widths.iter().map(|w| u32::from(*w)).sum::<u32>().max(1);
    let cols: Vec<u32> = widths
        .iter()
        .map(|w| u32::from(*w) * 9638 / total)
        .collect();
    let grid: String = cols
        .iter()
        .map(|w| format!("<w:gridCol w:w=\"{w}\"/>"))
        .collect();
    let border = "w:val=\"single\" w:sz=\"4\" w:space=\"0\" w:color=\"BFBFBF\"";
    let mut out = format!(
        "<w:tbl><w:tblPr><w:tblW w:w=\"5000\" w:type=\"pct\"/><w:tblBorders><w:top {border}/><w:bottom {border}/><w:insideH {border}/></w:tblBorders></w:tblPr><w:tblGrid>{grid}</w:tblGrid>"
    );
    for (i, row) in rows.iter().enumerate() {
        out.push_str(if i == 0 {
            "<w:tr><w:trPr><w:tblHeader/></w:trPr>"
        } else {
            "<w:tr>"
        });
        for (c, w) in cols.iter().enumerate() {
            let text = row.get(c).map(String::as_str).unwrap_or("");
            out.push_str(&format!(
                "<w:tc><w:tcPr><w:tcW w:w=\"{w}\" w:type=\"dxa\"/></w:tcPr>{}</w:tc>",
                para(text, None, i == 0, Some(17), false)
            ));
        }
        out.push_str("</w:tr>");
    }
    out.push_str("</w:tbl>");
    out.push_str(&para("", None, false, None, false));
    out
}

const STYLES: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
<w:docDefaults><w:rPrDefault><w:rPr><w:rFonts w:ascii="Calibri" w:hAnsi="Calibri" w:cs="Calibri"/><w:sz w:val="21"/><w:lang w:val="fr-CA"/></w:rPr></w:rPrDefault>
<w:pPrDefault><w:pPr><w:spacing w:after="80"/></w:pPr></w:pPrDefault></w:docDefaults>
<w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/></w:style>
<w:style w:type="paragraph" w:styleId="Title"><w:name w:val="Title"/><w:basedOn w:val="Normal"/><w:pPr><w:spacing w:after="160"/></w:pPr><w:rPr><w:b/><w:sz w:val="32"/></w:rPr></w:style>
<w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/><w:basedOn w:val="Normal"/><w:pPr><w:keepNext/><w:spacing w:before="240" w:after="80"/><w:outlineLvl w:val="0"/></w:pPr><w:rPr><w:b/><w:sz w:val="24"/></w:rPr></w:style>
<w:style w:type="paragraph" w:styleId="Notice"><w:name w:val="Notice"/><w:basedOn w:val="Normal"/><w:pPr><w:pBdr><w:left w:val="single" w:sz="18" w:space="6" w:color="D97700"/></w:pBdr></w:pPr><w:rPr><w:b/></w:rPr></w:style>
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

const DOC_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>
</Relationships>"#;

/// Renders the report as a Word document.
pub fn to_docx(r: &Report) -> Vec<u8> {
    let mut body = para(&r.title, Some("Title"), false, None, false);
    for b in &r.blocks {
        body.push_str(&match b {
            Block::Title(t) => para(t, Some("Title"), false, None, false),
            Block::Heading(t) => para(t, Some("Heading1"), false, None, false),
            Block::Paragraph(t) => para(t, None, false, None, false),
            Block::Small(t) => para(t, None, false, Some(15), true),
            Block::Notice(t) => para(t, Some("Notice"), false, None, false),
            Block::Fields(f) => f
                .iter()
                .map(|(k, v)| para(&format!("{k} : {v}"), None, false, None, false))
                .collect(),
            Block::Table { widths, rows } => table(widths, rows),
        });
    }
    body.push_str(&para(&r.footer, None, false, Some(15), true));
    let document = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>{body}<w:sectPr><w:pgSz w:w="11906" w:h="16838"/><w:pgMar w:top="1134" w:right="1134" w:bottom="1134" w:left="1134" w:header="567" w:footer="567" w:gutter="0"/></w:sectPr></w:body></w:document>"#
    );
    zip(&[
        ("[Content_Types].xml", CONTENT_TYPES.as_bytes().to_vec()),
        ("_rels/.rels", RELS.as_bytes().to_vec()),
        ("word/_rels/document.xml.rels", DOC_RELS.as_bytes().to_vec()),
        ("word/document.xml", document.into_bytes()),
        ("word/styles.xml", STYLES.as_bytes().to_vec()),
    ])
}

#[cfg(test)]
mod tests {
    #[test]
    fn escapes_xml() {
        assert_eq!(super::esc("a<b & \"c\"\u{1}"), "a&lt;b &amp; &quot;c&quot;");
    }
}
