#![allow(clippy::unwrap_used)] // test helpers may panic

use applicant::*;

fn ielts(s: u16, l: u16, r: u16, w: u16) -> LanguageTest {
    LanguageTest {
        test: TestKind::IeltsGeneral,
        test_date: IsoDate::parse("2026-01-15").unwrap(),
        speaking: s,
        listening: l,
        reading: r,
        writing: w,
    }
}

fn single() -> Applicant {
    Applicant {
        marital_status: MaritalStatus::NeverMarried,
        spouse: None,
        age: 29,
        education: EducationLevel::BachelorOrThreeYear,
        canadian_education: None,
        first_language: Some(ielts(70, 80, 70, 70)),
        second_language: None,
        canadian_work_years: 1,
        canadian_work_by_noc: vec![],
        foreign_work_years: 3,
        certificate_of_qualification: false,
        job_offer: None,
        provincial_nomination: false,
        sibling_in_canada: false,
        settlement_funds_cad: None,
        family_size: None,
    }
}

fn spouse(citizen: bool, accompanying: bool) -> Spouse {
    Spouse {
        is_canadian_citizen_or_pr: citizen,
        accompanying,
        education: Some(EducationLevel::Secondary),
        canadian_work_years: 0,
        language: None,
    }
}

#[test]
fn valid_single_applicant() {
    assert_eq!(single().validate(), Ok(()));
    assert!(!single().scored_with_spouse());
}

#[test]
fn spouse_rules() {
    let mut a = single();
    a.spouse = Some(spouse(false, true));
    assert_eq!(
        a.validate(),
        Err(vec![ValidationError::SpouseWithoutPartner])
    );

    a.marital_status = MaritalStatus::Married;
    assert_eq!(a.validate(), Ok(()));
    assert!(a.scored_with_spouse());

    a.spouse = Some(spouse(true, true));
    assert!(
        !a.scored_with_spouse(),
        "citizen/PR spouse scores as single"
    );
    a.spouse = Some(spouse(false, false));
    assert!(
        !a.scored_with_spouse(),
        "non-accompanying spouse scores as single"
    );

    a.spouse = None;
    assert_eq!(a.validate(), Err(vec![ValidationError::MissingSpouse]));
}

#[test]
fn impossible_work_history() {
    let mut a = single();
    a.age = 19;
    a.canadian_work_years = 6;
    a.foreign_work_years = 0;
    assert!(matches!(
        a.validate().unwrap_err()[..],
        [ValidationError::WorkYears {
            field: "canadian_work_years",
            ..
        }]
    ));
}

#[test]
fn language_score_ranges() {
    let mut a = single();
    a.first_language = Some(ielts(65, 95, 70, 67));
    let errs = a.validate().unwrap_err();
    assert_eq!(
        errs.len(),
        2,
        "listening 9.5 and writing 6.7 are invalid: {errs:?}"
    );
}

#[test]
fn second_language_must_differ() {
    let mut a = single();
    a.second_language = Some(ielts(60, 60, 60, 60));
    assert_eq!(
        a.validate(),
        Err(vec![ValidationError::SecondLanguageSameAsFirst])
    );

    a.second_language = Some(LanguageTest {
        test: TestKind::TcfCanada,
        test_date: IsoDate::parse("2026-01-15").unwrap(),
        speaking: 12,
        listening: 503,
        reading: 499,
        writing: 12,
    });
    assert_eq!(a.validate(), Ok(()));
}

#[test]
fn json_roundtrip_rejects_unknown_fields() {
    let a = single();
    let json = serde_json::to_string(&a).unwrap();
    let back: Applicant = serde_json::from_str(&json).unwrap();
    assert_eq!(a, back);
    let bad = json.replacen("{", "{\"nickname\":\"x\",", 1);
    assert!(serde_json::from_str::<Applicant>(&bad).is_err());
}

#[test]
fn noc_codes() {
    let teer = |s| Noc::parse(s).unwrap().offer_category();
    assert_eq!(teer("00012"), JobOfferNoc::Teer0Major00);
    assert_eq!(teer("10010"), JobOfferNoc::Teer0123);
    assert_eq!(teer("21231"), JobOfferNoc::Teer0123);
    assert_eq!(teer("64100"), JobOfferNoc::Teer45);
    for bad in ["2123", "212310", "2123a"] {
        assert!(Noc::parse(bad).is_none(), "{bad}");
    }
    assert!(serde_json::from_str::<Noc>("\"2123\"").is_err());
}

#[test]
fn canadian_work_by_noc_must_match_years() {
    let mut a = single();
    let w = |noc, months| NocMonths {
        noc: Noc::parse(noc).unwrap(),
        months,
    };
    a.canadian_work_by_noc = vec![w("21231", 8), w("10010", 6)];
    assert_eq!(a.validate(), Ok(()), "14 months = 1 year");
    a.canadian_work_by_noc = vec![w("21231", 30)];
    assert_eq!(
        a.validate(),
        Err(vec![ValidationError::CanadianWorkMismatch {
            months: 30,
            years: 1
        }])
    );
}

#[test]
fn dates() {
    let d = |s| IsoDate::parse(s).unwrap();
    assert!(IsoDate::parse("2026-02-29").is_none());
    assert!(IsoDate::parse("2028-02-29").is_some());
    assert!(IsoDate::parse("2026-04-31").is_none());
    assert_eq!(d("2028-02-29").plus_years(2), d("2030-02-28"));
    assert_eq!(d("2024-10-18").plus_years(2), d("2026-10-18"));
    assert_eq!(d("1997-10-19").years_until(&d("2026-10-18")), 28);
    assert_eq!(d("1997-10-18").years_until(&d("2026-10-18")), 29);
    assert_eq!(d("2026-10-18").years_until(&d("2026-10-17")), 0);
}
