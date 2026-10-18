//! Writes sample.pdf / sample.docx to the given directory (dev check).
use report::{Block, Report};

fn main() -> std::io::Result<()> {
    let dir = std::env::args().nth(1).unwrap_or_else(|| ".".into());
    let mut rows = vec![vec![
        "Élément".to_owned(),
        "Points".to_owned(),
        "Source".to_owned(),
    ]];
    for (l, p, c) in [
        (
            "Âge",
            "110",
            "criteries-scg · tableau 1, ligne 4 : 20 to 29 ans | 100 | 110",
        ),
        (
            "Niveau de scolarité",
            "120",
            "criteries-scg · tableau 2, ligne 5 : Baccalauréat OU autres programmes (trois ans ou plus) | 112 | 120",
        ),
        (
            "Première langue officielle",
            "124",
            "criteries-scg · tableau 3, ligne 6 : NCLC 9 | 29 | 31 (× 4 compétences)",
        ),
        (
            "Expérience de travail au Canada",
            "40",
            "criteries-scg · tableau 5, ligne 2 : 1 an | 35 | 40",
        ),
    ] {
        rows.push(vec![l.into(), p.into(), c.into()]);
    }
    let r = Report {
        title: "Évaluation d’admissibilité — Entrée express".into(),
        blocks: vec![
            Block::Fields(vec![
                ("Client".into(), "Hélène Tremblay-Côté".into()),
                ("Dossier".into(), "2026-041".into()),
                ("Date d’évaluation".into(), "2026-10-18".into()),
                ("Grille officielle".into(), "crs-2026-10-16".into()),
            ]),
            Block::Notice("BROUILLON — à valider par l’avocat avant toute utilisation.".into()),
            Block::Heading("Système de classement global : 482 points".into()),
            Block::Paragraph("A. Capital humain : 394 points. Calcul déterministe ; chaque ligne cite le tableau officiel d’IRCC.".into()),
            Block::Table { widths: vec![4, 1, 7], rows },
            Block::Small("Sources : criteries-scg.html (IRCC), relevé le 2026-10-18.".into()),
        ],
        footer: "LawyerTool 0.1.8 · document non chiffré : conservez-le en lieu sûr".into(),
    };
    std::fs::write(format!("{dir}/sample.pdf"), report::to_pdf(&r))?;
    std::fs::write(format!("{dir}/sample.docx"), report::to_docx(&r))?;
    Ok(())
}
