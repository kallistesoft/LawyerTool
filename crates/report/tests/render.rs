#![allow(clippy::unwrap_used)] // test code may panic

use report::{Block, Report, to_docx, to_pdf};

fn sample(rows: usize) -> Report {
    let mut table = vec![vec![
        "Élément".to_owned(),
        "Points".to_owned(),
        "Source".to_owned(),
    ]];
    for i in 0..rows {
        table.push(vec![
            format!("Ligne n° {i} — « première langue officielle » (NCLC 9)"),
            "124".into(),
            "criteries-scg · tableau 3, ligne 6 : NCLC 9 | 29 | 31".into(),
        ]);
    }
    Report {
        title: "Évaluation d’admissibilité — Entrée express".into(),
        blocks: vec![
            Block::Fields(vec![
                ("Client".into(), "Hélène Tremblay".into()),
                ("Date".into(), "2026-10-18".into()),
            ]),
            Block::Notice("Brouillon : à valider par l’avocat.".into()),
            Block::Heading("Système de classement global".into()),
            Block::Paragraph("Total : 482 points.".into()),
            Block::Table {
                widths: vec![5, 1, 5],
                rows: table,
            },
            Block::Small("Sources : pages officielles d’IRCC.".into()),
        ],
        footer: "LawyerTool · document non chiffré".into(),
    }
}

#[test]
fn pdf_is_well_formed() {
    let pdf = to_pdf(&sample(5));
    let text = String::from_utf8_lossy(&pdf);
    assert!(pdf.starts_with(b"%PDF-1.4"));
    assert!(text.trim_end().ends_with("%%EOF"));
    assert!(text.contains("/BaseFont /Helvetica /Encoding /WinAnsiEncoding"));
    // xref offsets (byte positions) point at the objects they name.
    let find = |needle: &[u8]| {
        pdf.windows(needle.len())
            .rposition(|w| w == needle)
            .unwrap()
    };
    let tail = &pdf[find(b"startxref\n") + 10..];
    let xref_at: usize = String::from_utf8_lossy(tail)
        .lines()
        .next()
        .unwrap()
        .parse()
        .unwrap();
    assert!(pdf[xref_at..].starts_with(b"xref"));
    let table = String::from_utf8_lossy(&pdf[xref_at..]).to_string();
    let entries: Vec<usize> = table
        .lines()
        .skip(3)
        .take_while(|l| l.ends_with(" n "))
        .map(|l| l[..10].parse().unwrap())
        .collect();
    assert!(!entries.is_empty());
    for (i, off) in entries.iter().enumerate() {
        assert!(
            pdf[*off..].starts_with(format!("{} 0 obj", i + 1).as_bytes()),
            "object {}",
            i + 1
        );
    }
    // Accents are encoded as WinAnsi octal escapes, not lost.
    assert!(text.contains("\\311valuation"), "É encoded");
}

#[test]
fn long_reports_paginate() {
    let one = String::from_utf8_lossy(&to_pdf(&sample(3)))
        .matches("/Type /Page ")
        .count();
    let many = String::from_utf8_lossy(&to_pdf(&sample(120)))
        .matches("/Type /Page ")
        .count();
    assert_eq!(one, 1);
    assert!(many >= 3, "{many} pages");
    assert!(
        String::from_utf8_lossy(&to_pdf(&sample(120))).contains(&format!("page {many} / {many}"))
    );
}

#[test]
fn docx_is_a_valid_package() {
    let d = to_docx(&sample(3));
    assert!(d.starts_with(b"PK\x03\x04"));
    let s = String::from_utf8_lossy(&d);
    for part in [
        "[Content_Types].xml",
        "_rels/.rels",
        "word/document.xml",
        "word/styles.xml",
    ] {
        assert!(s.contains(part), "{part}");
    }
    assert!(s.contains("Évaluation d’admissibilité"), "UTF-8 text kept");
    assert!(s.contains("<w:tblHeader/>"));
    // End of central directory with 5 entries.
    let eocd = d.windows(4).rposition(|w| w == b"PK\x05\x06").unwrap();
    assert_eq!(u16::from_le_bytes([d[eocd + 10], d[eocd + 11]]), 5);
}
