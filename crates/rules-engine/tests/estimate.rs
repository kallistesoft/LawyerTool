#![allow(clippy::unwrap_used)] // test helpers may panic

//! Estimates from self-declared levels must score exactly like real test
//! results at those levels (same core, levels assumed instead of converted).

use applicant::*;
use rules_engine::crs::{AssumedLevels, CrsError, estimate, score};
use rules_engine::grid::Grid;

const V0: &str = include_str!("../grids/crs-2026-10-16.json");

fn grid() -> Grid {
    let mut g: Grid = serde_json::from_str(V0).unwrap();
    rules_engine::rulings::apply(&mut g);
    g
}

fn d(s: &str) -> IsoDate {
    IsoDate::parse(s).unwrap()
}

/// CELPIP-G scores are CLB levels.
fn celpip(level: u16) -> LanguageTest {
    LanguageTest {
        test: TestKind::CelpipGeneral,
        test_date: d("2025-06-01"),
        speaking: level,
        listening: level,
        reading: level,
        writing: level,
    }
}

fn base() -> Applicant {
    Applicant {
        marital_status: MaritalStatus::NeverMarried,
        spouse: None,
        age: 32,
        education: EducationLevel::BachelorOrThreeYear,
        canadian_education: None,
        first_language: None,
        second_language: None,
        canadian_work_years: 0,
        canadian_work_by_noc: vec![],
        foreign_work_years: 3,
        certificate_of_qualification: false,
        job_offer: None,
        provincial_nomination: false,
        sibling_in_canada: true,
        eligibility: EligibilityFacts::default(),
    }
}

const ON: &str = "2026-10-18";

#[test]
fn english_estimate_equals_a_celpip_result_at_that_level() {
    let g = grid();
    let assumed = AssumedLevels {
        english: Some(8),
        ..AssumedLevels::default()
    };
    let est = estimate(&g, &base(), &assumed, &d(ON)).unwrap();
    let real = score(
        &g,
        &Applicant {
            first_language: Some(celpip(8)),
            ..base()
        },
        &d(ON),
    )
    .unwrap();
    assert_eq!(est.total, real.total);
    assert_eq!(est.core, real.core);
    assert_eq!(est.transferability, real.transferability);
    assert_eq!(est.additional.line("sibling in Canada"), Some(15));
}

#[test]
fn french_first_with_english_gets_the_french_bonus() {
    let g = grid();
    let fr9_en8 = AssumedLevels {
        french: Some(9),
        english: Some(8),
        spouse_english: None,
    };
    let r = estimate(&g, &base(), &fr9_en8, &d(ON)).unwrap();
    // NCLC 7+ in French and CLB 5+ in English: the strong-English bonus.
    let bonus = g.crs.additional.french_strong_english.points;
    assert_eq!(r.additional.line("French language skills"), Some(bonus));
    assert!(r.core.line("second official language").is_some());

    // English higher than French: English becomes the first language.
    let en_first = AssumedLevels {
        french: Some(6),
        english: Some(8),
        spouse_english: None,
    };
    let r2 = estimate(&g, &base(), &en_first, &d(ON)).unwrap();
    assert!(
        r2.additional.line("French language skills").is_none(),
        "NCLC 6 < 7"
    );
    assert!(
        r2.flags.is_empty(),
        "assumed levels raise no test divergences"
    );
}

#[test]
fn no_usable_level_means_no_estimate() {
    let none = AssumedLevels::default();
    assert_eq!(
        estimate(&grid(), &base(), &none, &d(ON)).unwrap_err(),
        CrsError::NoFirstLanguage
    );
}

#[test]
fn spouse_english_counts_only_with_an_accompanying_spouse() {
    let g = grid();
    let married = Applicant {
        marital_status: MaritalStatus::Married,
        spouse: Some(Spouse {
            is_canadian_citizen_or_pr: false,
            accompanying: true,
            education: Some(EducationLevel::Secondary),
            canadian_work_years: 0,
            language: None,
        }),
        ..base()
    };
    let with = AssumedLevels {
        french: Some(9),
        english: None,
        spouse_english: Some(8),
    };
    let without = AssumedLevels {
        spouse_english: None,
        ..with
    };
    let a = estimate(&g, &married, &with, &d(ON)).unwrap();
    let b = estimate(&g, &married, &without, &d(ON)).unwrap();
    assert!(a.spouse.line("spouse first official language").unwrap() > 0);
    assert!(b.spouse.line("spouse first official language").is_none());
    assert!(a.total > b.total);
}
