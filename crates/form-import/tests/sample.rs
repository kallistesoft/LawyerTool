#![allow(clippy::unwrap_used)] // test code may panic

//! The synthetic export (tests/fixtures/make_sample.py: the real form's
//! columns, invented respondents) becomes partial profiles with notes.

use applicant::{Credential, IsoDate, MaritalStatus};

const SAMPLE: &[u8] = include_bytes!("fixtures/sample-form.xlsx");

fn today() -> IsoDate {
    IsoDate::parse("2026-10-21").unwrap()
}

fn has(notes: &[String], part: &str) -> bool {
    notes.iter().any(|n| n.contains(part))
}

#[test]
fn every_respondent_becomes_a_partial_profile() {
    let p = form_import::parse(SAMPLE, "xlsx", &today()).unwrap();
    assert_eq!(p.responses.len(), 3);
    assert_eq!(p.skipped.len(), 1, "the row without a name");
    assert_eq!(p.skipped[0].row, 5);

    let alix = &p.responses[0];
    assert_eq!(alix.full_name, "TESTEUR Alix");
    assert_eq!(alix.email.as_deref(), Some("alix.testeur@example.com"));
    let a = &alix.applicant;
    assert_eq!(a.age, 32);
    assert_eq!(a.marital_status, MaritalStatus::NeverMarried);
    assert!(a.spouse.is_none());
    assert_eq!((a.canadian_work_years, a.foreign_work_years), (0, 5));
    assert!(a.first_language.is_none(), "no test scores in the form");
    assert!(
        !a.sibling_in_canada,
        "the question also covers parents and children"
    );
    assert_eq!(a.eligibility.relative_in_canada, Some(true));
    assert_eq!(
        a.eligibility.credential,
        Some(Credential::ForeignWithoutEca)
    );
    assert_eq!(
        (
            a.eligibility.family_size,
            a.eligibility.settlement_funds_cad
        ),
        (Some(1), Some(16_000))
    );
    assert_eq!(a.eligibility.intends_to_live_in_quebec, Some(false));
    assert_eq!(a.eligibility.authorized_to_work_in_canada, Some(false));
    assert!(has(&alix.notes, "réponse n° 101, reçue le 2026-10-18"));
    assert!(
        has(&alix.notes, "Niveau de scolarité à choisir") && has(&alix.notes, "Licence (bac+3)")
    );
    assert!(has(&alix.notes, "code CNP 31301 mentionné"));
    assert!(has(
        &alix.notes,
        "Test de français fait : saisir les résultats"
    ));
    assert!(has(&alix.notes, "seuls un frère ou une sœur"));
    assert!(!has(&alix.notes, "Santé"), "nothing declared");

    let camille = &p.responses[1];
    let c = &camille.applicant;
    assert_eq!(c.age, 38);
    assert_eq!(c.marital_status, MaritalStatus::Married);
    let s = c.spouse.as_ref().unwrap();
    assert!(s.accompanying && !s.is_canadian_citizen_or_pr && s.education.is_none());
    assert_eq!((c.canadian_work_years, c.foreign_work_years), (3, 1));
    assert_eq!(c.eligibility.intends_to_live_in_quebec, None);
    assert!(has(&camille.notes, "le Québec a son propre programme"));
    assert!(has(
        &camille.notes,
        "Statut actuel au Canada : « Travailleur temporaire (EIMT) »"
    ));
    assert!(has(
        &camille.notes,
        "Époux : scolarité à choisir (réponse « 3 années d'université »)"
    ));
    assert!(has(&camille.notes, "pas de date de naissance complète"));
    // Spouse education is required to score with a spouse: shown as a problem.
    assert!(c.validate().is_err());

    let dominique = &p.responses[2];
    let d = &dominique.applicant;
    assert_eq!(d.age, 45);
    assert_eq!(dominique.email, None, "not an email address");
    assert_eq!(d.eligibility.settlement_funds_cad, Some(0));
    assert!(has(
        &dominique.notes,
        "Santé : réponse « Diabète de type 1 »"
    ));
    assert!(d.validate().is_ok());

    // Keys are stable and distinct (re-imports are recognised).
    let again = form_import::parse(SAMPLE, "xlsx", &today()).unwrap();
    assert_eq!(again.responses[0].key, alix.key);
    assert_ne!(alix.key, camille.key);
}

#[test]
fn every_respondent_gets_an_estimate_with_its_assumptions() {
    use applicant::EducationLevel::*;
    use rules_engine::crs::AssumedLevels;

    let p = form_import::parse(SAMPLE, "xlsx", &today()).unwrap();
    let [alix, camille, dominique] = [&p.responses[0], &p.responses[1], &p.responses[2]];

    // Native French (NCLC 9), intermediate English (CLB 6), "Licence (bac+3)",
    // relative "Oui" counted as a sibling (ruling 2026-10-22).
    let e = &alix.estimate;
    assert_eq!(
        e.levels,
        AssumedLevels {
            french: Some(9),
            english: Some(6),
            spouse_english: None
        }
    );
    assert_eq!(e.applicant.education, BachelorOrThreeYear);
    assert!(e.applicant.sibling_in_canada);
    assert!(
        !alix.applicant.sibling_in_canada,
        "the profile itself stays conservative"
    );
    assert!(e.assumptions.iter().any(|a| a.contains("15 points")));

    // Married: spouse education from the list, beginner English = no points.
    let e = &camille.estimate;
    assert_eq!(e.levels.english, Some(8));
    assert_eq!(e.levels.spouse_english, None);
    assert_eq!(e.applicant.education, TwoYearPostSecondary);
    assert_eq!(
        e.applicant.spouse.as_ref().unwrap().education,
        Some(BachelorOrThreeYear)
    );
    assert!(
        e.applicant.validate().is_ok(),
        "the estimate is complete enough to score"
    );

    let e = &dominique.estimate;
    assert_eq!(
        e.levels,
        AssumedLevels {
            french: Some(9),
            english: None,
            spouse_english: None
        }
    );
    assert_eq!(e.applicant.education, MasterOrProfessional);
    assert!(e.stoppers.is_empty());
    assert!(e.checks.iter().any(|c| c.starts_with("Santé : « Diabète")));

    // Québec in the destination: to examine; no stopper for anyone here.
    assert!(
        camille
            .estimate
            .checks
            .iter()
            .any(|c| c.contains("Québec sélectionne"))
    );
    assert!(alix.estimate.stoppers.is_empty() && alix.estimate.checks.is_empty());

    // Each estimate scores on the current grid.
    let mut g: rules_engine::grid::Grid =
        serde_json::from_str(include_str!("../../rules-engine/grids/crs-2026-10-16.json")).unwrap();
    rules_engine::rulings::apply(&mut g);
    for r in &p.responses {
        let s =
            rules_engine::crs::estimate(&g, &r.estimate.applicant, &r.estimate.levels, &today())
                .unwrap_or_else(|e| panic!("{}: {e}", r.full_name));
        assert!(s.total > 200, "{}: {}", r.full_name, s.total);
    }
}

#[test]
fn other_files_are_refused_with_a_clear_message() {
    assert_eq!(
        form_import::parse(b"nom;prenom\nA;B\n", "csv", &today()).unwrap_err(),
        form_import::NOT_THIS_FORM
    );
    assert!(form_import::parse(b"not a zip", "xlsx", &today()).is_err());
    assert!(form_import::parse(b"", "csv", &today()).is_err());
}

#[test]
fn the_same_form_as_csv_imports_the_same_way() {
    let t = form_import::read_xlsx(SAMPLE).unwrap();
    let csv: String = t
        .iter()
        .map(|r| {
            r.iter()
                .map(|c| format!("\"{}\"", c.replace('"', "\"\"")))
                .collect::<Vec<_>>()
                .join(";")
        })
        .collect::<Vec<_>>()
        .join("\r\n");
    let a = form_import::parse(SAMPLE, "xlsx", &today()).unwrap();
    let b = form_import::parse(csv.as_bytes(), "csv", &today()).unwrap();
    assert_eq!(a.responses, b.responses);
}

mod hostile {
    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(300))]

        /// Random bytes and mutated exports never panic.
        #[test]
        fn never_panics(bytes in proptest::collection::vec(any::<u8>(), 0..4096), flip in 0usize..4000) {
            let today = applicant::IsoDate::parse("2026-10-21").unwrap();
            let _ = form_import::parse(&bytes, "xlsx", &today);
            let _ = form_import::parse(&bytes, "csv", &today);
            let mut sample = super::SAMPLE.to_vec();
            let i = flip % sample.len();
            sample[i] ^= 0x5a;
            let _ = form_import::parse(&sample, "xlsx", &today);
        }
    }
}

/// Manual check on a real export, read in place (never copied): prints
/// counts only, no answers. `FORM_SAMPLE=/path/export.xlsx cargo test -p
/// form-import -- --ignored real_export`.
#[test]
#[ignore = "needs FORM_SAMPLE"]
fn real_export() {
    let Ok(path) = std::env::var("FORM_SAMPLE") else {
        return;
    };
    let p = form_import::parse(&std::fs::read(path).unwrap(), "xlsx", &today()).unwrap();
    let ages = p.responses.iter().filter(|r| r.applicant.age > 0).count();
    let married = p
        .responses
        .iter()
        .filter(|r| r.applicant.spouse.is_some())
        .count();
    let funds = p
        .responses
        .iter()
        .filter(|r| r.applicant.eligibility.settlement_funds_cad.is_some())
        .count();
    let emails = p.responses.iter().filter(|r| r.email.is_some()).count();
    let valid = p
        .responses
        .iter()
        .filter(|r| r.applicant.validate().is_ok())
        .count();
    println!(
        "responses {} skipped {} | age {ages} married {married} funds {funds} email {emails} valid {valid} | notes per response {:?}",
        p.responses.len(),
        p.skipped.len(),
        p.responses
            .iter()
            .map(|r| r.notes.len())
            .collect::<Vec<_>>()
    );
}
