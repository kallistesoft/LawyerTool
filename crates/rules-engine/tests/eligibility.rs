#![allow(clippy::unwrap_used)] // test helpers may panic

//! Hand-worked eligibility scenarios. Hours: days × 30 h / 7 (weekly cap).

use applicant::*;
use rules_engine::eligibility::{Check, Program, ProgramResult, Status, assess, hours};
use rules_engine::grid::Grid;

const V0: &str = include_str!("../grids/crs-2026-10-16.json");
const ON: &str = "2026-10-18";

fn grid() -> Grid {
    let mut g: Grid = serde_json::from_str(V0).unwrap();
    rules_engine::rulings::apply(&mut g);
    g
}

fn d(s: &str) -> IsoDate {
    IsoDate::parse(s).unwrap()
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

fn job(noc: &str, canada: bool, start: &str, end: &str, h: u8) -> WorkPeriod {
    WorkPeriod {
        noc: Noc::parse(noc).unwrap(),
        in_canada: canada,
        start: d(start),
        end: d(end),
        hours_per_week: h,
        paid: true,
        self_employed: false,
        full_time_student: false,
        authorized: true,
        qualified_in_trade: Some(true),
    }
}

/// Single, 30, bachelor with ECA, IELTS CLB 9, 3 years abroad in NOC 21231.
fn fsw_base() -> Applicant {
    Applicant {
        marital_status: MaritalStatus::NeverMarried,
        spouse: None,
        age: 30,
        education: EducationLevel::BachelorOrThreeYear,
        canadian_education: None,
        first_language: Some(lt(TestKind::IeltsGeneral, [70, 80, 70, 70])),
        second_language: None,
        canadian_work_years: 0,
        canadian_work_by_noc: vec![],
        foreign_work_years: 3,
        certificate_of_qualification: false,
        job_offer: None,
        provincial_nomination: false,
        sibling_in_canada: false,
        eligibility: EligibilityFacts {
            work_history: vec![job("21231", false, "2021-01-01", "2023-12-31", 40)],
            credential: Some(Credential::ForeignWithEca),
            canadian_study_two_years: Some(false),
            spouse_canadian_study_two_years: None,
            spouse_canadian_work_one_year: None,
            relative_in_canada: Some(false),
            authorized_to_work_in_canada: Some(false),
            fsw_arranged_employment: Some(false),
            intends_to_live_in_quebec: Some(false),
            settlement_funds_cad: Some(20_000),
            family_size: Some(1),
        },
    }
}

fn result(a: &Applicant, p: Program) -> ProgramResult {
    assess(&grid(), a, &d(ON))
        .unwrap()
        .into_iter()
        .find(|r| r.program == p)
        .unwrap()
}

fn check(r: &ProgramResult, id: &str) -> Check {
    r.requirement(id)
        .unwrap_or_else(|| panic!("no requirement {id}: {r:#?}"))
        .check
}

#[test]
fn weekly_cap_across_concurrent_jobs() {
    // Two 20 h jobs over 365 days → 40 h/week capped at 30 → 365 × 30 / 7 = 1564.
    let jobs = [
        job("21231", true, "2025-01-01", "2025-12-31", 20),
        job("10010", true, "2025-01-01", "2025-12-31", 20),
    ];
    assert_eq!(hours(&jobs, &d("2000-01-01"), &d("2030-01-01"), 30), 1564);
    // Window clips the period.
    assert_eq!(
        hours(&jobs[..1], &d("2025-07-01"), &d("2025-12-31"), 30),
        184 * 20 / 7
    );
}

#[test]
fn fsw_pass_mark_boundary() {
    // Language 6 × 4 = 24, education 21, experience: 1095 d × 30 / 7 = 4692 h → 3 yrs → 11,
    // age 30 → 12, arranged 0, adaptability 0 → 68.
    let r = result(&fsw_base(), Program::Fsw);
    let p = r.fsw_points.clone().unwrap();
    assert_eq!((p.min, p.max), (68, 68), "{:?}", p.lines);
    assert_eq!(r.status, Status::Eligible, "{r:#?}");
    assert_eq!(check(&r, "admissible"), Check::LawyerCheck);

    let mut a = fsw_base();
    a.age = 36; // 11 → 67: still passes
    assert_eq!(result(&a, Program::Fsw).fsw_points.unwrap().min, 67);
    assert_eq!(result(&a, Program::Fsw).status, Status::Eligible);
    a.age = 37; // 10 → 66
    let r = result(&a, Program::Fsw);
    assert_eq!(check(&r, "selection_points"), Check::NotMet);
    assert_eq!(r.status, Status::NotEligible);

    // Unknown arranged employment at 66: could reach 76 (10 + 5 adaptability, capped 10).
    a.eligibility.fsw_arranged_employment = None;
    let r = result(&a, Program::Fsw);
    let p = r.fsw_points.clone().unwrap();
    assert_eq!((p.min, p.max), (66, 66 + 10 + 5));
    assert_eq!(r.status, Status::NeedsInput);
}

#[test]
fn fsw_minimums() {
    let mut a = fsw_base();
    a.first_language = Some(lt(TestKind::IeltsGeneral, [70, 80, 70, 55])); // writing CLB 6
    assert_eq!(check(&result(&a, Program::Fsw), "language"), Check::NotMet);

    let mut a = fsw_base();
    a.eligibility.credential = Some(Credential::ForeignWithoutEca);
    assert_eq!(check(&result(&a, Program::Fsw), "education"), Check::NotMet);

    // 11 months only, and in TEER 4: not enough skilled work.
    let mut a = fsw_base();
    a.eligibility.work_history = vec![
        job("21231", false, "2025-01-01", "2025-11-30", 40),
        job("64100", false, "2020-01-01", "2024-12-31", 40),
    ];
    assert_eq!(
        check(&result(&a, Program::Fsw), "experience"),
        Check::NotMet
    );

    // Experience older than 10 years does not count.
    let mut a = fsw_base();
    a.eligibility.work_history = vec![job("21231", false, "2010-01-01", "2016-01-01", 40)];
    assert_eq!(
        check(&result(&a, Program::Fsw), "experience"),
        Check::NotMet
    );

    let mut a = fsw_base();
    a.eligibility.work_history.clear();
    assert_eq!(
        check(&result(&a, Program::Fsw), "experience"),
        Check::NeedsInput
    );
}

#[test]
fn funds_rules() {
    let mut a = fsw_base();
    a.eligibility.settlement_funds_cad = Some(15_000); // needs 15 263 for 1
    assert_eq!(check(&result(&a, Program::Fsw), "funds"), Check::NotMet);

    a.job_offer = Some(JobOffer {
        noc: Noc::parse("21231").unwrap(),
    });
    a.eligibility.authorized_to_work_in_canada = None;
    assert_eq!(
        check(&result(&a, Program::Fsw), "funds"),
        Check::NeedsInput,
        "exempt if authorized"
    );
    a.eligibility.authorized_to_work_in_canada = Some(true);
    assert_eq!(check(&result(&a, Program::Fsw), "funds"), Check::Met);
}

fn cec_base() -> Applicant {
    let mut a = fsw_base();
    a.canadian_work_years = 1;
    a.foreign_work_years = 0;
    // 2024-01-01..2025-06-30: 547 d × 30 / 7 = 2344 h.
    a.eligibility.work_history = vec![job("21231", true, "2024-01-01", "2025-06-30", 37)];
    a
}

#[test]
fn cec() {
    let r = result(&cec_base(), Program::Cec);
    assert_eq!(r.status, Status::Eligible, "{r:#?}");
    assert!(
        r.requirement("experience")
            .unwrap()
            .detail
            .starts_with("2344 h")
    );

    // Funds are not required for CEC.
    let mut a = cec_base();
    a.eligibility.settlement_funds_cad = None;
    assert!(result(&a, Program::Cec).requirement("funds").is_none());

    // TEER 1 job needs CLB 7; CLB 6 fails.
    let mut a = cec_base();
    a.first_language = Some(lt(TestKind::IeltsGeneral, [55, 55, 50, 55]));
    assert_eq!(check(&result(&a, Program::Cec), "language"), Check::NotMet);
    // TEER 2 job: CLB 5 is enough.
    a.eligibility.work_history[0].noc = Noc::parse("22220").unwrap();
    assert_eq!(check(&result(&a, Program::Cec), "language"), Check::Met);
    // Mixed TEER 1 + 2 below CLB 7 needs the lawyer.
    a.eligibility.work_history = vec![
        job("21231", true, "2024-01-01", "2024-09-30", 30),
        job("22220", true, "2024-10-01", "2025-06-30", 30),
    ];
    assert_eq!(
        check(&result(&a, Program::Cec), "language"),
        Check::LawyerCheck
    );

    // Self-employment, full-time-student work, unauthorized work and work older than 3 years don't count.
    for tweak in [
        (|w: &mut WorkPeriod| w.self_employed = true) as fn(&mut WorkPeriod),
        |w| w.full_time_student = true,
        |w| w.authorized = false,
        |w| {
            w.start = IsoDate::parse("2021-01-01").unwrap();
            w.end = IsoDate::parse("2023-06-30").unwrap();
        },
    ] {
        let mut a = cec_base();
        tweak(&mut a.eligibility.work_history[0]);
        assert_eq!(
            check(&result(&a, Program::Cec), "experience"),
            Check::NotMet
        );
    }
}

fn fst_base() -> Applicant {
    let mut a = fsw_base();
    a.first_language = Some(lt(TestKind::CelpipGeneral, [5, 5, 4, 4]));
    a.certificate_of_qualification = true;
    // Machinists (72100): 2022-01-01..2024-12-31 = 1096 d × 30 / 7 = 4697 h.
    a.eligibility.work_history = vec![job("72100", false, "2022-01-01", "2024-12-31", 40)];
    a.eligibility.family_size = Some(2);
    a.eligibility.settlement_funds_cad = Some(25_000);
    a
}

#[test]
fn fst() {
    let r = result(&fst_base(), Program::Fst);
    assert_eq!(r.status, Status::Eligible, "{r:#?}");

    let mut a = fst_base();
    a.eligibility.work_history[0].noc = Noc::parse("72600").unwrap(); // excluded sub-major group 726
    assert_eq!(
        check(&result(&a, Program::Fst), "experience"),
        Check::NotMet
    );

    let mut a = fst_base();
    a.certificate_of_qualification = false;
    assert_eq!(
        check(&result(&a, Program::Fst), "offer_or_certificate"),
        Check::NotMet
    );

    let mut a = fst_base();
    a.first_language = Some(lt(TestKind::CelpipGeneral, [4, 5, 4, 4]));
    assert_eq!(
        check(&result(&a, Program::Fst), "language"),
        Check::NotMet,
        "speaking needs CLB 5"
    );

    let mut a = fst_base();
    a.eligibility.work_history[0].qualified_in_trade = None;
    assert_eq!(
        check(&result(&a, Program::Fst), "qualified_in_trade"),
        Check::LawyerCheck
    );

    // FSW language minimum is not met by this profile (CLB 4–5).
    assert_eq!(
        check(&result(&fst_base(), Program::Fsw), "language"),
        Check::NotMet
    );
}

#[test]
fn quebec_and_expired_tests() {
    let mut a = cec_base();
    a.eligibility.intends_to_live_in_quebec = Some(true);
    assert_eq!(result(&a, Program::Cec).status, Status::NotEligible);
    a.eligibility.intends_to_live_in_quebec = None;
    assert_eq!(result(&a, Program::Cec).status, Status::NeedsInput);

    let mut a = cec_base();
    a.first_language.as_mut().unwrap().test_date = d("2024-06-01");
    assert_eq!(
        check(&result(&a, Program::Cec), "language"),
        Check::NotMet,
        "older than 2 years"
    );
}
