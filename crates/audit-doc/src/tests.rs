#![allow(clippy::unwrap_used)]

//! One synthetic respondent per track (no real client data).

use super::*;
use report::rich::Item;

fn ctx() -> Context {
    let mut g: rules_engine::grid::Grid =
        serde_json::from_str(include_str!("../../rules-engine/grids/crs-2026-10-16.json")).unwrap();
    rules_engine::rulings::apply(&mut g);
    Context {
        today: IsoDate::parse("2026-10-22").unwrap(),
        french_cutoff: Some(435),
        funds: g.programs.map(|p| p.funds),
    }
}

fn form(age: u8, pairs: &[(&str, &str)]) -> FormRecord {
    FormRecord {
        answers: pairs
            .iter()
            .map(|(k, v)| ((*k).into(), (*v).into()))
            .collect(),
        age: Some(age),
        lawyer: Lawyer::default(),
    }
}

/// All the text of the document, items separated by newlines; highlighted
/// runs are wrapped in ⟦ ⟧ so tests can see what the lawyer must fill.
fn text(d: &Doc) -> String {
    fn runs(out: &mut String, rs: &[Run]) {
        for r in rs {
            if r.highlight {
                out.push('⟦');
                out.push_str(&r.text);
                out.push('⟧');
            } else {
                out.push_str(&r.text);
            }
        }
    }
    fn para(out: &mut String, p: &Para) {
        runs(out, &p.runs);
        out.push('\n');
    }
    let mut out = String::new();
    for i in &d.items {
        match i {
            Item::Para(p) => para(&mut out, p),
            Item::Banner { lines, .. } => runs(&mut out, lines),
            Item::Heading { text, .. } => out.push_str(&format!("# {}\n", text.to_uppercase())),
            Item::Table { header, rows, .. } => {
                for row in header.iter().chain(rows) {
                    for c in row {
                        for p in &c.paras {
                            para(&mut out, p);
                        }
                        out.push_str("| ");
                    }
                    out.push('\n');
                }
            }
            Item::Boxed(ps) => ps.iter().for_each(|p| para(&mut out, p)),
            Item::PageBreak => out.push_str("---\n"),
        }
    }
    out
}

const SINGLE_QC: &[(&str, &str)] = &[
    ("name", "TESTEUR Alix"),
    ("nationality", "Française"),
    ("residence", "France"),
    ("marital", "Célibataire"),
    ("job", "Directrice commerciale (CNP 60010)"),
    ("experience", "4 ans"),
    ("education", "Licence (bac+3)"),
    ("eca", "Évaluation des diplômes pas encore faite"),
    ("french_test", "Non"),
    ("french", "Langue maternelle"),
    ("english", "Débutant"),
    (
        "funds",
        "4 personnes : J'ai au moins 30 000 $ CAD d'économies\u{a0}",
    ),
    ("province", "Québec"),
    ("relative", "Non"),
    ("criminal", "Jamais condamné ou suspecté"),
    ("health", "Non"),
];

#[test]
fn single_french_speaker_aiming_at_quebec() {
    let mut f = form(32, SINGLE_QC);
    f.lawyer.noc = Some("60010".into());
    f.lawyer.noc_validated = true;
    f.lawyer.regulated = Some("n’est pas réglementée".into());
    let t = text(&build("TESTEUR Alix", &f, &ctx()));
    for want in [
        "AUDIT ÉCLAIR D’ÉLIGIBILITÉ",
        "Date de l’évaluation – 22 octobre 2026",
        "32 ans — Célibataire",
        "Citoyenneté Française (Résidence : France)",
        "Atout Majeur (Mobilité Francophone C16)",
        "Licence (bac+3) (ÉDE pas encore faite)",
        "(CNP 60010)",
        "Métier très qualifié (FEER 0)",
        "Français (Langue maternelle – Test officiel à passer) / Anglais (débutant)",
        "Au moins 30\u{202f}000 $ CAD d'économies pour 4",
        "Immigration complexe et longue",
        "Remarque préalable",
        "📍 AU QUÉBEC",
        "(20%)",
        "ÉLEVÉ",
        "(80%)",
        "~435 pts",
        "doit être amélioré hors Québec.",
        "Nouveau Brunswick (NB)",
        "📌 ÉTAPE 1",
        "TEST DE FRANÇAIS",
        "non opportun",
        "n’est pas réglementée",
        "(mobilité francophone donc hors QUÉBEC)",
        "📌 ÉTAPE 4",
        "INSTALLATION &",
        "FÉLICITATIONS : UN EXCELLENT POTENTIEL !",
        "CNP 60010 (FEER 0)",
        "Inscrivez-vous au PVT",
        "Élargir vos horizons hors Québec",
        "VOTRE CONSULTATION STRATÉGIQUE",
        "Vanessa POUGHON – Avocate",
        "ANNEXE",
    ] {
        assert!(t.contains(want), "missing {want:?}\n{t}");
    }
    for absent in [
        "CONJOINT",
        "Requérant + conjoint",
        "Après 35 ans",
        "PRÉLIMINAIRE",
        "⟦CNP",
        "Point à examiner",
    ] {
        assert!(!t.contains(absent), "unexpected {absent:?}");
    }
}

#[test]
fn married_with_unknown_cnp_aiming_at_alberta() {
    let f = form(
        41,
        &[
            ("nationality", "Française"),
            ("residence", "France"),
            ("marital", "Marié ou en union de fait"),
            ("spouse_age", "43 ans"),
            ("spouse_nationality", "Française"),
            ("spouse_education", "Il/elle n'a pas le baccalauréat"),
            ("spouse_english", "Débutant"),
            ("spouse_job", "Chauffeur"),
            ("spouse_experience", "5 ans et plus"),
            ("spouse_test", "Non"),
            ("job", "Ostéopathe"),
            ("experience", "2 ans"),
            ("education", "Bac+5"),
            ("eca", "Évaluation des diplômes faite ou en cours"),
            ("french_test", "Oui"),
            ("french", "Langue maternelle"),
            ("english", "Débutant"),
            ("province", "Alberta"),
            ("relative", "Oui"),
        ],
    );
    let t = text(&build("Camille EXEMPLE", &f, &ctx()));
    for want in [
        "Après 35 ans, on perd des points pour l'Entrée express.",
        "ÉDE faite ou en cours",
        "⟦à valider⟧",
        "⟦CNP À DÉFINIR IMPÉRATIVEMENT⟧",
        "Test réalisé",
        "Non renseigné",
        "pour 2 personnes pour la RP",
        "doit être amélioré pour l'Alberta.",
        "AUDIT SUR-MESURE & IMPACT STRATÉGIQUE DU CONJOINT",
        "CONJOINT : 43 ans, française, il/elle n'a pas le baccalauréat, anglais débutant, chauffeur, ancienneté 5 ans et plus.",
        "25%",
        "PAS UTILE",
        "Requérant + conjoint",
        "⟦[réglementée ou non : à vérifier selon la CNP]⟧",
        "TEST D’ANGLAIS",
        "Votre proche au Canada",
        "⟦CNP [à déterminer]⟧",
    ] {
        assert!(t.contains(want), "missing {want:?}\n{t}");
    }
    for absent in [
        "📍 AU QUÉBEC",
        "Inscrivez-vous au PVT",
        "TEST DE FRANÇAIS\n",
    ] {
        assert!(!t.contains(absent), "unexpected {absent:?}");
    }
}

#[test]
fn low_skilled_bilingual_young_single() {
    let mut f = form(
        27,
        &[
            ("marital", "Célibataire"),
            ("job", "Créatrice de contenu"),
            ("experience", "5 ans et plus"),
            ("french", "Langue maternelle"),
            ("english", "Bilingue"),
            ("province", "Ontario"),
            ("funds", "Je n'ai pas les fonds disponibles."),
            ("health", "Asthme sévère"),
        ],
    );
    f.lawyer.noc = Some("55109".into());
    f.lawyer.noc_validated = true;
    let t = text(&build("Dominique FICTIF", &f, &ctx()));
    for want in [
        "Métier peu qualifié (FEER 5) et non admissible à l'Entrée express",
        "MODÉRÉ",
        "(50%)",
        "après de l’expérience dans un métier qualifié",
        "anglais bilingue est un très gros atout",
        "PRÉLIMINAIRE",
        "📌 ÉTAPE 1\n\nTEST DE FRANÇAIS",
        "CRÉATION D'XP",
        "DE L'EXPÉRIENCE À LA MOBILISATION",
        "incroyable atout",
        "Non conforme",
        "⟦Point à examiner avant envoi (admissibilité) : ⟧⟦Santé : « Asthme sévère »⟧",
    ] {
        assert!(t.contains(want), "missing {want:?}\n{t}");
    }
    assert!(
        !t.contains("Nouveau Brunswick (NB)"),
        "bilingual and NB not targeted"
    );
    assert!(!t.contains("EXCELLENT POTENTIEL"));
}

#[test]
fn no_funds_uses_the_ircc_amount_for_the_household() {
    let mut pairs = SINGLE_QC.to_vec();
    pairs.retain(|(k, _)| *k != "funds");
    pairs.push(("funds", "Je n'ai pas les fonds disponibles."));
    let t = text(&build("A B", &form(30, &pairs), &ctx()));
    let needed = ctx().funds.unwrap().required(1).unwrap().0;
    assert!(
        t.contains(&format!(
            "Au moins {} $ CAD d'économies requis pour 1 personne",
            money(needed)
        )),
        "{t}"
    );
    assert!(t.contains("Consolider votre autonomie financière"));
}

#[test]
fn names_dates_and_docx() {
    assert_eq!(file_name("TESTEUR Alix"), "Audit_TESTEUR.docx");
    assert_eq!(file_name("Exemple Camille"), "Audit_EXEMPLE.docx");
    assert_eq!(file_name("Brouëssé Hélène "), "Audit_BROUESSE.docx");
    assert_eq!(file_name("LE TESTEUR Argentina"), "Audit_LE-TESTEUR.docx");
    assert_eq!(file_name("Aimy"), "Audit_AIMY.docx");
    assert_eq!(file_name(""), "Audit_CLIENT.docx");
    assert_eq!(
        long_date(&IsoDate::parse("2026-01-05").unwrap()),
        "5 janvier 2026"
    );
    let bytes = report::rich::to_docx(&build("TESTEUR Alix", &form(32, SINGLE_QC), &ctx()));
    assert!(bytes.starts_with(b"PK") && bytes.len() > 10_000);
}
