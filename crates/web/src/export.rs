//! Assessment report export (PDF / Word). The engine renders the file; the
//! page writes it into the output folder chosen in Paramètres (never
//! overwriting) or, if none is set, where the user chooses in the browser's
//! "Save as" dialog. Refused until every mandatory review flag is reviewed.

use applicant::Applicant;
use report::{Block, Report};
use rules_engine::crs::{CrsResult, Flag, Section};
use rules_engine::eligibility::{Check, Program, ProgramResult, Status};
use rules_engine::grid::Cite;
use serde::{Deserialize, Serialize};
use vault_file::{Identity, Vault};

use crate::clients::{Assessment, assess_inner};
use crate::{CommandError, Res, time};

/// French label of a CRS line (engine labels are stable English keys).
pub fn line_label(l: &'static str) -> &'static str {
    match l {
        "age" => "Âge",
        "education" => "Niveau de scolarité",
        "first official language" => "Première langue officielle",
        "second official language" => "Deuxième langue officielle",
        "Canadian work experience" => "Expérience de travail au Canada",
        "spouse education" => "Scolarité de l’époux",
        "spouse first official language" => "Langue de l’époux",
        "spouse Canadian work experience" => "Expérience au Canada de l’époux",
        "foreign work experience" => "Expérience de travail à l’étranger",
        "certificate of qualification" => "Certificat de compétence",
        "provincial nomination" => "Désignation provinciale",
        "Canadian education" => "Études au Canada",
        "sibling in Canada" => "Frère ou sœur au Canada",
        "French language skills" => "Compétences en français",
        "qualifying job offer" => "Offre d’emploi admissible",
        other => other,
    }
}

fn requirement_label(id: &str) -> &str {
    match id {
        "experience" => "Expérience de travail",
        "noc_duties" => "Fonctions de la CNP",
        "language" => "Compétences linguistiques",
        "education" => "Études",
        "selection_points" => "Grille de sélection (67 points)",
        "funds" => "Preuve de fonds",
        "outside_quebec" => "Établissement hors Québec",
        "admissible" => "Admissibilité au Canada",
        "offer_or_certificate" => "Offre d’emploi ou certificat",
        "qualified_in_trade" => "Qualification dans le métier",
        other => other,
    }
}

fn program_label(p: Program) -> &'static str {
    match p {
        Program::Fsw => "Programme des travailleurs qualifiés (fédéral)",
        Program::Cec => "Catégorie de l’expérience canadienne",
        Program::Fst => "Programme des travailleurs de métiers spécialisés (fédéral)",
    }
}

fn status_label(s: Status) -> &'static str {
    match s {
        Status::Eligible => "Admissible (vérifications de l’avocat à confirmer)",
        Status::NotEligible => "Non admissible",
        Status::NeedsInput => "Renseignements manquants",
    }
}

fn check_label(c: Check) -> &'static str {
    match c {
        Check::Met => "Satisfait",
        Check::NotMet => "Non satisfait",
        Check::NeedsInput => "À compléter",
        Check::LawyerCheck => "À vérifier par l’avocat",
    }
}

fn cite(c: &Cite) -> String {
    let loc = match (c.table, c.row) {
        (Some(t), Some(r)) => format!(" · tableau {t}, ligne {r}"),
        _ => String::new(),
    };
    format!("{}{loc} : {}", c.source, c.text)
}

fn cites(cs: &[Cite]) -> String {
    cs.iter().map(cite).collect::<Vec<_>>().join("\n")
}

fn section_table(s: &Section) -> Block {
    let mut rows = vec![vec![
        "Élément".to_owned(),
        "Points".to_owned(),
        "Source officielle".to_owned(),
    ]];
    for l in &s.lines {
        rows.push(vec![
            line_label(l.label).to_owned(),
            l.points.to_string(),
            cites(&l.cites),
        ]);
    }
    Block::Table {
        widths: vec![3, 1, 8],
        rows,
    }
}

/// Mandatory lawyer-review ids in a CRS result.
pub fn review_ids(crs: &CrsResult) -> Vec<String> {
    crs.flags
        .iter()
        .filter_map(|f| match f {
            Flag::ManualReview { id, .. } => Some(id.clone()),
            _ => None,
        })
        .collect()
}

pub fn build(
    identity: &Identity,
    _profile: &Applicant,
    a: &Assessment,
    reviewed_on: &str,
) -> Report {
    let mut blocks = vec![
        Block::Fields(
            [
                Some(("Client".to_owned(), identity.full_name.clone())),
                identity.file_reference.clone().map(|f| ("Dossier".to_owned(), f)),
                Some(("Date d’évaluation".to_owned(), a.on.clone())),
                Some(("Grille officielle".to_owned(), a.grid_id.clone().unwrap_or_else(|| "aucune".into()))),
                Some(("Généré le".to_owned(), reviewed_on.to_owned())),
            ]
            .into_iter()
            .flatten()
            .collect(),
        ),
        Block::Notice(
            "BROUILLON — évaluation automatisée à valider par l’avocat. Ne constitue pas un avis juridique avant son approbation.".into(),
        ),
    ];

    match &a.crs {
        Ok(c) => {
            let reviews: Vec<String> = c
                .flags
                .iter()
                .filter_map(|f| match f {
                    Flag::ManualReview { id, reason } => {
                        Some(format!("{id} (révisé par l’avocat) : {reason}"))
                    }
                    _ => None,
                })
                .collect();
            if !reviews.is_empty() {
                blocks.push(Block::Heading(
                    "Points ayant exigé une révision de l’avocat".into(),
                ));
                for r in reviews {
                    blocks.push(Block::Paragraph(r));
                }
            }
            for f in &c.flags {
                if let Flag::ExpiredLanguageTest { which } = f {
                    blocks.push(Block::Notice(format!(
                        "Test de langue expiré à la date d’évaluation : {which}."
                    )));
                }
            }
            blocks.push(Block::Heading(format!(
                "Système de classement global : {} points ({} époux)",
                c.total,
                if c.with_spouse { "avec" } else { "sans" }
            )));
            let mut sections = vec![("A. Facteurs de base / capital humain", Some(&c.core))];
            if c.with_spouse {
                sections.push((
                    "B. Facteurs liés à l’époux ou conjoint de fait",
                    Some(&c.spouse),
                ));
            }
            sections.push((
                "C. Transférabilité des compétences",
                Some(&c.transferability),
            ));
            sections.push(("D. Points supplémentaires", Some(&c.additional)));
            sections.push(("Intégration au marché du travail", c.labour_market.as_ref()));
            for (title, s) in sections {
                if let Some(s) = s {
                    blocks.push(Block::Paragraph(format!("{title} : {} points", s.subtotal)));
                    if !s.lines.is_empty() {
                        blocks.push(section_table(s));
                    }
                }
            }
        }
        Err(e) => blocks.push(Block::Notice(format!("Pointage SCG non calculé : {e}"))),
    }

    blocks.push(Block::Heading("Admissibilité aux programmes".into()));
    match &a.eligibility {
        Ok(programs) => {
            for p in programs {
                blocks.push(Block::Paragraph(format!(
                    "{} : {}",
                    program_label(p.program),
                    status_label(p.status)
                )));
                if let Some(f) = &p.fsw_points {
                    let pts = if f.min == f.max {
                        f.min.to_string()
                    } else {
                        format!("{}–{}", f.min, f.max)
                    };
                    blocks.push(Block::Small(format!(
                        "Grille de sélection : {pts} / 100 (seuil {})",
                        f.pass_mark
                    )));
                }
                blocks.push(requirements_table(p));
            }
        }
        Err(e) => blocks.push(Block::Notice(format!("Admissibilité non évaluée : {e}"))),
    }

    blocks.push(Block::Small(
        "Calcul déterministe selon les pages officielles d’IRCC enregistrées (voir les sources citées). Les divergences entre sources officielles sont tranchées par l’avocat et signalées ci-dessus.".into(),
    ));

    Report {
        title: "Évaluation d’admissibilité — Entrée express".into(),
        blocks,
        footer: format!(
            "LawyerTool {} · document non chiffré : conservez-le en lieu sûr",
            env!("CARGO_PKG_VERSION")
        ),
    }
}

fn requirements_table(p: &ProgramResult) -> Block {
    let mut rows = vec![vec![
        "Exigence".to_owned(),
        "État".to_owned(),
        "Détail".to_owned(),
        "Source officielle".to_owned(),
    ]];
    for r in &p.requirements {
        rows.push(vec![
            requirement_label(r.id).to_owned(),
            check_label(r.check).to_owned(),
            r.detail.clone(),
            cites(&r.cites),
        ]);
    }
    Block::Table {
        widths: vec![3, 2, 5, 6],
        rows,
    }
}

#[derive(Deserialize)]
pub struct ExportRequest {
    id: i64,
    on: Option<String>,
    /// "pdf" or "docx".
    format: String,
    /// Review-flag ids the lawyer has checked.
    reviewed: Vec<String>,
}

/// A rendered report for the page to save.
#[derive(Serialize)]
pub struct ExportFile {
    /// Suggested file name (the page adds -2, -3… if it is taken).
    pub name: String,
    pub mime: &'static str,
    /// "PDF" or "Document Word", for the "Save as" dialog.
    pub kind: &'static str,
    #[serde(skip)]
    pub bytes: Vec<u8>,
}

pub fn export(v: &Vault, request: ExportRequest) -> Res<ExportFile> {
    let assessment = assess_inner(v, request.id, request.on.clone())?;

    if let Ok(c) = &assessment.crs {
        let missing: Vec<String> = review_ids(c)
            .into_iter()
            .filter(|id| !request.reviewed.contains(id))
            .collect();
        if !missing.is_empty() {
            return Err(CommandError::new(
                "review_required",
                format!(
                    "Révision obligatoire avant l’export : {}.",
                    missing.join(", ")
                ),
            ));
        }
    }

    let r = v
        .client_get(request.id)
        .ok_or_else(|| CommandError::new("not_found", "Client introuvable."))?;
    if !r.notes.is_empty() {
        return Err(CommandError::new(
            "review_required",
            "Profil importé du formulaire : complétez-le, puis cliquez « Tout est complété » dans le profil avant l’export.",
        ));
    }
    let a: Applicant = serde_json::from_str(&r.profile)
        .map_err(|_| CommandError::new("corrupt", "Profil illisible."))?;
    let report = build(&r.identity, &a, &assessment, time::today().as_str());
    let (bytes, ext, mime, kind) = match request.format.as_str() {
        "pdf" => (report::to_pdf(&report), "pdf", "application/pdf", "PDF"),
        "docx" => (
            report::to_docx(&report),
            "docx",
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            "Document Word",
        ),
        _ => return Err(CommandError::new("invalid", "Format inconnu.")),
    };
    let stem = r
        .identity
        .file_reference
        .as_deref()
        .filter(|f| !f.is_empty())
        .map_or_else(|| format!("client-{}", request.id), sanitize);
    Ok(ExportFile {
        name: format!("Evaluation-{stem}-{}.{ext}", assessment.on),
        mime,
        kind,
        bytes,
    })
}

/// File-name-safe version of a file reference.
fn sanitize(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    /// Every label the engine emits has a French translation.
    #[test]
    fn every_crs_label_is_translated() {
        let engine = include_str!("../../rules-engine/src/crs.rs");
        let mut labels = Vec::new();
        for chunk in engine.split(".push(").skip(1) {
            let t = chunk.trim_start();
            if let Some(rest) = t.strip_prefix('"') {
                labels.push(rest.split('"').next().unwrap_or_default().to_owned());
            }
        }
        assert!(labels.len() >= 14, "{labels:?}");
        for l in labels {
            let l: &'static str = Box::leak(l.into_boxed_str());
            assert_ne!(super::line_label(l), l, "no French label for {l:?}");
        }
    }

    #[test]
    fn full_profile_renders_to_pdf_and_docx() {
        let a: applicant::Applicant =
            serde_json::from_str(include_str!("../tests/fixtures/ui-full-applicant.json"))
                .unwrap_or_else(|e| panic!("{e}"));
        let on = applicant::IsoDate::parse("2026-10-18").unwrap_or_else(|| unreachable!());
        let grids = crate::clients::tests_grids();
        let g = rules_engine::grid::in_force(grids, &on).unwrap_or_else(|| unreachable!());
        let assessment = crate::clients::Assessment {
            on: on.as_str().into(),
            grid_id: Some(g.id.clone()),
            crs: rules_engine::crs::score(g, &a, &on).map_err(|e| e.to_string()),
            eligibility: rules_engine::eligibility::assess(g, &a, &on).map_err(|e| e.to_string()),
            draws: vec![],
        };
        // This profile is French-first with TEF: D-003 must be reviewed before export.
        let crs = assessment.crs.as_ref().unwrap_or_else(|e| panic!("{e}"));
        assert!(super::review_ids(crs).contains(&"D-003".to_owned()));
        let id = vault_file::Identity {
            full_name: "Hélène Tremblay".into(),
            email: None,
            phone: None,
            file_reference: Some("2026-041".into()),
        };
        let r = super::build(&id, &a, &assessment, "2026-10-18");
        let pdf = report::to_pdf(&r);
        assert!(pdf.starts_with(b"%PDF") && pdf.len() > 2000);
        assert!(report::to_docx(&r).starts_with(b"PK"));
    }

    #[test]
    fn sanitize_file_reference() {
        assert_eq!(super::sanitize("2026/041 é"), "2026_041__");
    }

    #[test]
    fn export_is_refused_until_reviews_are_checked() {
        let mut app = crate::App::new(vault_file::KdfParams::INSECURE_FAST_FOR_TESTS);
        app.create("a long test passphrase")
            .unwrap_or_else(|e| panic!("{}", e.message()));
        let profile: serde_json::Value =
            serde_json::from_str(include_str!("../tests/fixtures/ui-full-applicant.json"))
                .unwrap_or_else(|e| panic!("{e}"));
        app.call(
            "client_save",
            &serde_json::json!({ "request": {
                "id": null,
                "identity": { "full_name": "Hélène Tremblay", "email": null, "phone": null, "file_reference": "2026/041" },
                "profile": profile,
            }}),
        )
        .unwrap_or_else(|e| panic!("{}", e.message()));
        let req = |reviewed: &[&str]| super::ExportRequest {
            id: 1,
            on: Some("2026-10-18".into()),
            format: "pdf".into(),
            reviewed: reviewed.iter().map(|s| (*s).to_owned()).collect(),
        };
        let e = app
            .export_report(req(&[]))
            .err()
            .unwrap_or_else(|| unreachable!());
        assert_eq!(e.code, "review_required");
        let f = app
            .export_report(req(&["D-003"]))
            .unwrap_or_else(|e| panic!("{}", e.message()));
        assert_eq!(f.name, "Evaluation-2026_041-2026-10-18.pdf");
        assert!(f.bytes.starts_with(b"%PDF"));

        // An imported profile with open notes cannot be exported.
        app.vault_mut()
            .and_then(|v| Ok(v.client_set_notes(1, vec!["à compléter".into()])?))
            .unwrap_or_else(|e| panic!("{}", e.message()));
        let e = app
            .export_report(req(&["D-003"]))
            .err()
            .unwrap_or_else(|| unreachable!());
        assert!(e.message().contains("Tout est complété"));
        app.call("client_clear_notes", &serde_json::json!({ "id": 1 }))
            .unwrap_or_else(|e| panic!("{}", e.message()));
        assert!(app.export_report(req(&["D-003"])).is_ok());
    }
}
