use applicant::*;

fn ielts(s: u16, l: u16, r: u16, w: u16) -> LanguageTest {
    LanguageTest {
        test: TestKind::IeltsGeneral,
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
