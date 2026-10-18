//! Writes the golden CRS profiles (synthetic, never client data) with the
//! calculator form answers for each, ready to be recorded on the live IRCC
//! calculator:
//!   cargo run -p oracle-tests --example golden -- crates/rules-engine/fixtures/crs/golden
//! Existing `calculator` / `recorded` fields are preserved.

use std::path::Path;

use applicant::*;
use oracle_tests::answers;
use rules_engine::grid::Grid;
use serde_json::{Value, json};

type Res<T> = Result<T, Box<dyn std::error::Error>>;

fn d(s: &str) -> IsoDate {
    IsoDate::parse(s).unwrap_or_else(|| panic!("bad date {s}"))
}

fn lt(test: TestKind, [s, l, r, w]: [u16; 4]) -> LanguageTest {
    LanguageTest {
        test,
        test_date: d("2025-06-01"),
        speaking: s,
        listening: l,
        reading: r,
        writing: w,
    }
}

// Scores by CLB/NCLC level for each test (post-2023 TEF scale).
fn ielts(clb: u8) -> LanguageTest {
    let s = match clb {
        10 => [75, 85, 80, 75],
        9 => [70, 80, 70, 70],
        8 => [65, 75, 65, 65],
        7 => [60, 60, 60, 60],
        6 => [55, 55, 50, 55],
        5 => [50, 50, 40, 50],
        4 => [40, 45, 35, 40],
        _ => [30, 30, 30, 30],
    };
    lt(TestKind::IeltsGeneral, s)
}

fn celpip(clb: u8) -> LanguageTest {
    lt(TestKind::CelpipGeneral, [u16::from(clb); 4])
}

fn tef(nclc: u8) -> LanguageTest {
    let s = match nclc {
        10 => [560, 550, 550, 560],
        9 => [520, 510, 510, 520],
        8 => [500, 470, 470, 480],
        7 => [470, 440, 440, 450],
        6 => [430, 400, 400, 400],
        5 => [400, 360, 360, 350],
        _ => [330, 310, 310, 270],
    };
    lt(TestKind::TefCanada, s)
}

fn single() -> Applicant {
    Applicant {
        marital_status: MaritalStatus::NeverMarried,
        spouse: None,
        age: 29,
        education: EducationLevel::BachelorOrThreeYear,
        canadian_education: None,
        first_language: Some(ielts(9)),
        second_language: None,
        canadian_work_years: 1,
        canadian_work_by_noc: vec![],
        foreign_work_years: 3,
        certificate_of_qualification: false,
        job_offer: None,
        provincial_nomination: false,
        sibling_in_canada: false,
        eligibility: EligibilityFacts::default(),
    }
}

fn spouse(
    citizen: bool,
    accompanying: bool,
    education: EducationLevel,
    years: u8,
    language: Option<LanguageTest>,
) -> Spouse {
    Spouse {
        is_canadian_citizen_or_pr: citizen,
        accompanying,
        education: Some(education),
        canadian_work_years: years,
        language,
    }
}

fn married(mut a: Applicant, s: Spouse) -> Applicant {
    a.marital_status = MaritalStatus::Married;
    a.spouse = Some(s);
    a
}

fn with(mut a: Applicant, f: impl FnOnce(&mut Applicant)) -> Applicant {
    f(&mut a);
    a
}

fn profiles() -> Vec<(&'static str, &'static str, Applicant)> {
    use EducationLevel::*;
    vec![
        (
            "01-single-base",
            "Single, 29, bachelor, IELTS CLB 9, 1 yr Canada, 3 yrs abroad",
            single(),
        ),
        (
            "02-age-17",
            "Age 17: no age points",
            with(single(), |a| {
                a.age = 17;
                a.canadian_work_years = 0;
                a.foreign_work_years = 0
            }),
        ),
        (
            "03-age-18",
            "Age 18",
            with(single(), |a| {
                a.age = 18;
                a.canadian_work_years = 0;
                a.foreign_work_years = 0
            }),
        ),
        (
            "04-age-20",
            "Age 20 (start of the 20–29 plateau)",
            with(single(), |a| {
                a.age = 20;
                a.foreign_work_years = 2
            }),
        ),
        ("05-age-30", "Age 30", with(single(), |a| a.age = 30)),
        ("06-age-35", "Age 35", with(single(), |a| a.age = 35)),
        (
            "07-age-44",
            "Age 44 (last year with points)",
            with(single(), |a| a.age = 44),
        ),
        (
            "08-age-45",
            "Age 45: no age points",
            with(single(), |a| a.age = 45),
        ),
        (
            "09-married-accompanying",
            "Married, spouse accompanying: master, CELPIP 9, 2 yrs Canada",
            married(
                with(single(), |a| a.age = 32),
                spouse(false, true, MasterOrProfessional, 2, Some(celpip(9))),
            ),
        ),
        (
            "10-spouse-citizen",
            "Married to a citizen/PR: scored as single",
            married(
                single(),
                spouse(true, true, MasterOrProfessional, 2, Some(celpip(9))),
            ),
        ),
        (
            "11-common-law-not-accompanying",
            "Common-law partner not accompanying: scored as single",
            with(
                married(single(), spouse(false, false, Secondary, 0, None)),
                |a| a.marital_status = MaritalStatus::CommonLaw,
            ),
        ),
        (
            "12-less-than-secondary-clb4",
            "Less than secondary, IELTS CLB 4, no work",
            with(single(), |a| {
                a.education = LessThanSecondary;
                a.first_language = Some(ielts(4));
                a.canadian_work_years = 0;
                a.foreign_work_years = 0
            }),
        ),
        (
            "13-secondary",
            "Secondary diploma only",
            with(single(), |a| a.education = Secondary),
        ),
        (
            "14-one-year",
            "One-year post-secondary",
            with(single(), |a| a.education = OneYearPostSecondary),
        ),
        (
            "15-two-credentials-clb7",
            "Two credentials, CLB 7, 2 yrs Canada",
            with(single(), |a| {
                a.education = TwoOrMoreCredentials;
                a.first_language = Some(ielts(7));
                a.canadian_work_years = 2
            }),
        ),
        (
            "16-doctorate-transfer-cap",
            "PhD, CLB 10, 5 yrs Canada, 3 yrs abroad, certificate: transferability capped at 100",
            with(single(), |a| {
                a.age = 34;
                a.education = Doctorate;
                a.first_language = Some(celpip(10));
                a.canadian_work_years = 5;
                a.certificate_of_qualification = true
            }),
        ),
        (
            "17-second-language-cap-with-spouse",
            "With spouse, English CLB 10 + French NCLC 10: second language capped at 22",
            married(
                with(single(), |a| {
                    a.first_language = Some(celpip(10));
                    a.second_language = Some(tef(10))
                }),
                spouse(false, true, BachelorOrThreeYear, 0, None),
            ),
        ),
        (
            "18-french-first-no-english",
            "French NCLC 7 first, no English: French bonus 25",
            with(single(), |a| a.first_language = Some(tef(7))),
        ),
        (
            "19-french-first-english-clb5",
            "French NCLC 7 first, IELTS CLB 5: French bonus 50",
            with(single(), |a| {
                a.first_language = Some(tef(7));
                a.second_language = Some(ielts(5))
            }),
        ),
        (
            "20-english-first-french-nclc7",
            "English CLB 9 first, French NCLC 7 second: French bonus 50",
            with(single(), |a| a.second_language = Some(tef(7))),
        ),
        (
            "21-tcf-nclc4-d001",
            "TCF first, listening at NCLC 4 (D-001)",
            with(single(), |a| {
                a.first_language = Some(LanguageTest {
                    test: TestKind::TcfCanada,
                    test_date: d("2025-06-01"),
                    speaking: 12,
                    listening: 340,
                    reading: 500,
                    writing: 12,
                })
            }),
        ),
        (
            "22-french-first-pte-d004",
            "French NCLC 7 first, PTE Core CLB 5 (D-004)",
            with(single(), |a| {
                a.first_language = Some(tef(7));
                a.second_language = Some(lt(TestKind::PteCore, [51, 39, 42, 51]))
            }),
        ),
        (
            "23-second-language-nclc4-d002",
            "English CLB 9 first, French NCLC 4 second (D-002)",
            with(single(), |a| a.second_language = Some(tef(4))),
        ),
        (
            "24-certificate-clb5",
            "Certificate of qualification with CLB 5–6",
            with(single(), |a| {
                a.first_language = Some(ielts(6));
                a.certificate_of_qualification = true
            }),
        ),
        (
            "25-certificate-clb7",
            "Certificate of qualification with CLB 7+",
            with(single(), |a| {
                a.first_language = Some(ielts(7));
                a.certificate_of_qualification = true
            }),
        ),
        (
            "26-nomination-cap",
            "Provincial nomination + sibling + 3-yr Canadian education: additional capped at 600",
            with(single(), |a| {
                a.provincial_nomination = true;
                a.sibling_in_canada = true;
                a.canadian_education = Some(CanadianEducation::ThreeYearOrMore)
            }),
        ),
        (
            "27-canadian-education-short",
            "1–2 year Canadian credential",
            with(single(), |a| {
                a.canadian_education = Some(CanadianEducation::OneOrTwoYear)
            }),
        ),
        (
            "28-job-offer-teer00",
            "Job offer NOC 00012 (TEER 0, group 00): 0 points before 2026-11-03",
            with(single(), |a| {
                a.job_offer = Some(JobOffer {
                    noc: Noc::parse("00012").unwrap_or_else(|| unreachable!()),
                })
            }),
        ),
        (
            "29-minimum",
            "Minimum: age 45, less than secondary, CLB 4, no work, nothing else",
            with(single(), |a| {
                a.age = 45;
                a.education = LessThanSecondary;
                a.first_language = Some(ielts(4));
                a.canadian_work_years = 0;
                a.foreign_work_years = 0
            }),
        ),
    ]
}

fn main() -> Res<()> {
    let dir = std::env::args().nth(1).ok_or("usage: golden <dir>")?;
    let dir = Path::new(&dir);
    std::fs::create_dir_all(dir)?;
    let mut grid: Grid =
        serde_json::from_str(include_str!("../../rules-engine/grids/crs-2026-10-16.json"))?;
    rules_engine::rulings::apply(&mut grid);
    for (id, description, a) in profiles() {
        a.validate().map_err(|e| format!("{id}: {e:?}"))?;
        let form = answers(&grid, &a).ok_or(format!("{id}: not expressible in the calculator"))?;
        let path = dir.join(format!("{id}.json"));
        let old: Value = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or(Value::Null);
        let doc = json!({
            "id": id,
            "description": description,
            "grid": grid.id,
            "applicant": a,
            "form": form,
            "calculator": old.get("calculator").cloned().unwrap_or(Value::Null),
            "recorded": old.get("recorded").cloned().unwrap_or(Value::Null),
        });
        std::fs::write(&path, serde_json::to_string_pretty(&doc)? + "\n")?;
    }
    Ok(())
}
