//! Spreadsheet reading: the first worksheet of an `.xlsx` file, or a `.csv`
//! file, as rows of text cells. Form files are untrusted: every size is
//! capped, only plain (stored or deflated) zip entries are read, and the
//! XML parser never resolves external entities.

use std::collections::HashMap;

use quick_xml::events::Event;

/// Largest decompressed zip entry read (bytes).
const MAX_ENTRY: usize = 16 * 1024 * 1024;
/// Most zip entries looked at.
const MAX_ENTRIES: usize = 200;
/// Most rows and columns kept.
pub const MAX_ROWS: usize = 2_000;
pub const MAX_COLS: usize = 400;
/// Longest cell kept (characters); longer answers are cut.
pub const MAX_CELL: usize = 4_000;

/// Rows of cells (the first row holds the headers).
pub type Table = Vec<Vec<String>>;

fn u16_at(b: &[u8], i: usize) -> Option<usize> {
    Some(u16::from_le_bytes(b.get(i..i + 2)?.try_into().ok()?) as usize)
}

fn u32_at(b: &[u8], i: usize) -> Option<usize> {
    usize::try_from(u32::from_le_bytes(b.get(i..i + 4)?.try_into().ok()?)).ok()
}

/// The entries of a zip archive, by name (only the ones `want` accepts).
fn unzip(bytes: &[u8], want: impl Fn(&str) -> bool) -> Result<HashMap<String, Vec<u8>>, String> {
    let bad = || "fichier Excel endommagé".to_owned();
    // End of central directory: in the last 64 KiB + 22 bytes.
    let start = bytes.len().saturating_sub(65_557);
    let eocd = (start..bytes.len().saturating_sub(21))
        .rev()
        .find(|&i| bytes.get(i..i + 4) == Some(&[0x50, 0x4b, 0x05, 0x06]))
        .ok_or_else(bad)?;
    let count = u16_at(bytes, eocd + 10).ok_or_else(bad)?;
    let mut at = u32_at(bytes, eocd + 16).ok_or_else(bad)?;
    let mut out = HashMap::new();
    for _ in 0..count.min(MAX_ENTRIES) {
        if bytes.get(at..at + 4) != Some(&[0x50, 0x4b, 0x01, 0x02]) {
            return Err(bad());
        }
        let flags = u16_at(bytes, at + 8).ok_or_else(bad)?;
        let method = u16_at(bytes, at + 10).ok_or_else(bad)?;
        let compressed = u32_at(bytes, at + 20).ok_or_else(bad)?;
        let size = u32_at(bytes, at + 24).ok_or_else(bad)?;
        let name_len = u16_at(bytes, at + 28).ok_or_else(bad)?;
        let extra_len = u16_at(bytes, at + 30).ok_or_else(bad)?;
        let comment_len = u16_at(bytes, at + 32).ok_or_else(bad)?;
        let local = u32_at(bytes, at + 42).ok_or_else(bad)?;
        let name = String::from_utf8_lossy(bytes.get(at + 46..at + 46 + name_len).ok_or_else(bad)?)
            .into_owned();
        at += 46 + name_len + extra_len + comment_len;
        if !want(&name) {
            continue;
        }
        if flags & 1 != 0 {
            return Err("fichier Excel protégé par mot de passe".into());
        }
        if size > MAX_ENTRY {
            return Err("fichier Excel trop volumineux".into());
        }
        if bytes.get(local..local + 4) != Some(&[0x50, 0x4b, 0x03, 0x04]) {
            return Err(bad());
        }
        let data_at = local
            + 30
            + u16_at(bytes, local + 26).ok_or_else(bad)?
            + u16_at(bytes, local + 28).ok_or_else(bad)?;
        let data = bytes.get(data_at..data_at + compressed).ok_or_else(bad)?;
        let plain = match method {
            0 => data.to_vec(),
            8 => miniz_oxide::inflate::decompress_to_vec_with_limit(data, MAX_ENTRY)
                .map_err(|_| bad())?,
            _ => return Err("compression du fichier Excel non prise en charge".into()),
        };
        out.insert(name, plain);
    }
    Ok(out)
}

/// Column index of a cell reference such as `"AB12"` (0-based).
fn column(r: &str) -> Option<usize> {
    let mut n = 0usize;
    let mut any = false;
    for c in r.chars().take_while(char::is_ascii_uppercase) {
        n = n
            .checked_mul(26)?
            .checked_add(usize::from(c as u8 - b'A') + 1)?;
        any = true;
    }
    any.then(|| n - 1)
}

fn push_text(cell: &mut String, s: &str) {
    let room = MAX_CELL.saturating_sub(cell.chars().count());
    cell.extend(s.chars().take(room));
}

/// `&name;` / `&#n;` as text.
fn entity(name: &str) -> String {
    quick_xml::escape::unescape(&format!("&{name};"))
        .map(|c| c.into_owned())
        .unwrap_or_default()
}

/// Text of every `<si>` in sharedStrings.xml (phonetic runs skipped).
fn shared_strings(xml: &str) -> Result<Vec<String>, String> {
    let mut r = quick_xml::Reader::from_str(xml);
    let (mut out, mut cur) = (Vec::new(), String::new());
    let (mut in_t, mut in_rph) = (false, false);
    loop {
        match r
            .read_event()
            .map_err(|_| "texte du fichier Excel illisible".to_owned())?
        {
            Event::Start(e) => match e.local_name().as_ref() {
                "si" => cur.clear(),
                "t" => in_t = true,
                "rPh" => in_rph = true,
                _ => {}
            },
            Event::End(e) => match e.local_name().as_ref() {
                "si" => out.push(std::mem::take(&mut cur)),
                "t" => in_t = false,
                "rPh" => in_rph = false,
                _ => {}
            },
            Event::Empty(e) if e.local_name().as_ref() == "si" => out.push(String::new()),
            Event::Text(t) if in_t && !in_rph => push_text(&mut cur, &t.xml10_content()),
            Event::GeneralRef(g) if in_t && !in_rph => {
                push_text(&mut cur, &entity(&g.xml10_content()))
            }
            Event::Eof => break,
            _ => {}
        }
        if out.len() > MAX_ROWS * MAX_COLS {
            return Err("fichier Excel trop volumineux".into());
        }
    }
    Ok(out)
}

/// Cells of a worksheet, as rows (missing cells are empty strings).
fn sheet(xml: &str, strings: &[String]) -> Result<Table, String> {
    let mut r = quick_xml::Reader::from_str(xml);
    let mut rows: Table = Vec::new();
    let mut row: Vec<String> = Vec::new();
    let (mut col, mut kind, mut val) = (0usize, String::new(), String::new());
    let (mut in_v, mut in_t, mut in_row) = (false, false, false);
    loop {
        match r
            .read_event()
            .map_err(|_| "feuille du fichier Excel illisible".to_owned())?
        {
            Event::Start(e) => match e.local_name().as_ref() {
                "row" => {
                    in_row = true;
                    row.clear();
                }
                "c" if in_row => {
                    col = e
                        .try_get_attribute("r")
                        .ok()
                        .flatten()
                        .and_then(|a| {
                            a.normalized_value(quick_xml::XmlVersion::Implicit1_0)
                                .ok()
                                .and_then(|v| column(&v))
                        })
                        .unwrap_or(row.len());
                    kind = e
                        .try_get_attribute("t")
                        .ok()
                        .flatten()
                        .and_then(|a| {
                            a.normalized_value(quick_xml::XmlVersion::Implicit1_0)
                                .ok()
                                .map(|v| v.into_owned())
                        })
                        .unwrap_or_default();
                    val.clear();
                }
                "v" => in_v = true,
                "t" => in_t = true,
                _ => {}
            },
            Event::End(e) => match e.local_name().as_ref() {
                "row" => {
                    in_row = false;
                    rows.push(std::mem::take(&mut row));
                    if rows.len() >= MAX_ROWS {
                        break;
                    }
                }
                "c" if in_row && col < MAX_COLS => {
                    let text = match kind.as_str() {
                        "s" => val
                            .trim()
                            .parse::<usize>()
                            .ok()
                            .and_then(|i| strings.get(i))
                            .cloned()
                            .unwrap_or_default(),
                        "b" => (if val.trim() == "1" { "TRUE" } else { "FALSE" }).to_owned(),
                        _ => std::mem::take(&mut val),
                    };
                    if row.len() <= col {
                        row.resize(col + 1, String::new());
                    }
                    row[col] = text;
                }
                "v" => in_v = false,
                "t" => in_t = false,
                _ => {}
            },
            Event::Text(t) if in_v || in_t => push_text(&mut val, &t.xml10_content()),
            Event::GeneralRef(g) if in_v || in_t => {
                push_text(&mut val, &entity(&g.xml10_content()))
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(rows)
}

/// The first worksheet of an `.xlsx` file.
pub fn read_xlsx(bytes: &[u8]) -> Result<Table, String> {
    let files = unzip(bytes, |n| {
        n == "xl/sharedStrings.xml" || (n.starts_with("xl/worksheets/") && n.ends_with(".xml"))
    })?;
    let text = |name: &str| {
        files
            .get(name)
            .map(|b| String::from_utf8_lossy(b).into_owned())
    };
    let strings = match text("xl/sharedStrings.xml") {
        Some(x) => shared_strings(&x)?,
        None => vec![],
    };
    let first = text("xl/worksheets/sheet1.xml").or_else(|| {
        let mut names: Vec<&String> = files
            .keys()
            .filter(|n| n.starts_with("xl/worksheets/"))
            .collect();
        names.sort();
        names.first().and_then(|n| text(n))
    });
    sheet(
        &first.ok_or("aucune feuille dans le fichier Excel")?,
        &strings,
    )
}

/// A `.csv` file (UTF-8, optional BOM; comma or semicolon, by the header).
pub fn read_csv(bytes: &[u8]) -> Result<Table, String> {
    let text = std::str::from_utf8(bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes))
        .map_err(|_| "fichier CSV illisible (encodage UTF-8 attendu)".to_owned())?;
    let header = text.lines().next().unwrap_or_default();
    let sep = if header.matches(';').count() > header.matches(',').count() {
        ';'
    } else {
        ','
    };
    let mut rows: Table = Vec::new();
    let (mut row, mut cell) = (Vec::new(), String::new());
    let mut quoted = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match (quoted, c) {
            (true, '"') if chars.peek() == Some(&'"') => {
                chars.next();
                push_text(&mut cell, "\"");
            }
            (true, '"') => quoted = false,
            (true, c) => push_text(&mut cell, c.encode_utf8(&mut [0; 4])),
            (false, '"') if cell.is_empty() => quoted = true,
            (false, c) if c == sep => {
                if row.len() < MAX_COLS {
                    row.push(std::mem::take(&mut cell));
                }
                cell.clear();
            }
            (false, '\r') => {}
            (false, '\n') => {
                if row.len() < MAX_COLS {
                    row.push(std::mem::take(&mut cell));
                }
                cell.clear();
                rows.push(std::mem::take(&mut row));
                if rows.len() >= MAX_ROWS {
                    return Ok(rows);
                }
            }
            (false, c) => push_text(&mut cell, c.encode_utf8(&mut [0; 4])),
        }
    }
    if !cell.is_empty() || !row.is_empty() {
        row.push(cell);
        rows.push(row);
    }
    Ok(rows)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn column_letters() {
        assert_eq!(column("A1"), Some(0));
        assert_eq!(column("Z9"), Some(25));
        assert_eq!(column("AA2"), Some(26));
        assert_eq!(column("CY1"), Some(102));
        assert_eq!(column("12"), None);
    }

    #[test]
    fn csv_quotes_semicolons_and_newlines() {
        let t = read_csv("\u{feff}a;b;c\r\n1;\"x; \"\"y\"\"\nz\";3\n".as_bytes()).unwrap();
        assert_eq!(t, vec![vec!["a", "b", "c"], vec!["1", "x; \"y\"\nz", "3"]]);
        let t = read_csv(b"a,b\n1,2").unwrap();
        assert_eq!(t[1], vec!["1", "2"]);
    }

    #[test]
    fn shared_strings_with_runs_entities_and_phonetics() {
        let x = r#"<sst><si><t>A &amp; B</t></si><si><r><t>Hé</t></r><r><t>lène</t></r><rPh><t>x</t></rPh></si><si/></sst>"#;
        assert_eq!(shared_strings(x).unwrap(), vec!["A & B", "Hélène", ""]);
    }

    #[test]
    fn sheet_cells_by_reference() {
        let x = r#"<worksheet><sheetData><row r="1"><c r="A1" t="s"><v>0</v></c><c r="C1" t="inlineStr"><is><t>c</t></is></c></row><row r="2"><c r="B2"><v>46313.5</v></c></row></sheetData></worksheet>"#;
        let t = sheet(x, &["a".to_owned()]).unwrap();
        assert_eq!(t, vec![vec!["a", "", "c"], vec!["", "46313.5"]]);
    }

    #[test]
    fn not_a_zip_and_truncated_zips_are_refused() {
        assert!(read_xlsx(b"%PDF-1.7").is_err());
        assert!(
            read_xlsx(&[
                0x50, 0x4b, 0x05, 0x06, 0, 0, 0, 0, 0, 0, 9, 0, 0, 0, 0, 0, 255, 255, 255, 255, 0,
                0
            ])
            .is_err()
        );
    }
}
