#![allow(clippy::unwrap_used)] // test helpers may panic

//! Hand-computed profiles. Each expected value is worked out from the grid
//! tables in the comment above it; the automated oracle (calculator JS) and
//! the golden profiles cover the rest.

use applicant::*;
use rules_engine::crs::{CrsError, Flag, score};
use rules_engine::grid::Grid;

const V0: &str = include_str!("../grids/crs-2026-10-16.json");
const V1: &str = include_str!("../grids/crs-2026-11-03.json");

fn load(json: &str) -> Grid {
    let mut g: Grid = serde_json::from_str(json).unwrap();
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

fn base() -> Applicant {
    Applicant {
        marital_status: MaritalStatus::NeverMarried,
        spouse: None,
        age: 29,
        education: EducationLevel::BachelorOrThreeYear,
        canadian_education: None,
        first_language: Some(lt(TestKind::IeltsGeneral, [70, 80, 70, 70])), // CLB 9
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

const TODAY: &str = "2026-10-18";

#[test]
fn single_bachelor_clb9() {
    // Core: age 29 → 110, bachelor → 120, CLB 9 → 31 × 4 = 124, 1 yr Canada → 40 = 394.
    // Transfer: education 25 (CLB 9) + 13 (1 yr) = 38; foreign 3+ yrs: 50 + 25 = 75 → cap 50.
    // Total 394 + 0 + 88 + 0 = 482.
    let r = score(&load(V0), &base(), &d(TODAY)).unwrap();
    assert_eq!(r.core.subtotal, 394);
    assert_eq!(r.transferability.line("education"), Some(38));
    assert_eq!(r.transferability.line("foreign work experience"), Some(50));
    assert_eq!(
        (r.spouse.subtotal, r.additional.subtotal, r.total),
        (0, 0, 482)
    );
    assert!(r.flags.is_empty(), "{:?}", r.flags);
    assert!(
        r.core.lines.iter().all(|l| !l.cites.is_empty()),
        "every line is cited"
    );
}

#[test]
fn married_french_first_with_extras() {
    // With spouse. Core: age 35 → 70, master → 126, NCLC 7 → 16 × 4 = 64,
    // English CLB 7 second → 3 × 4 = 12, no Canadian work → 0 = 272.
    // Spouse: bachelor 8 + CLB 7 → 3 × 4 = 12 + 1 yr → 5 = 25.
    // Transfer: education 25 (master, NCLC 7) + 0; foreign 1–2 yrs 13 + 0 = 38.
    // Additional: Canadian 3-yr education 30 + sibling 15 + French 50 = 95.
    // Total 272 + 25 + 38 + 95 = 430.
    let mut a = base();
    a.marital_status = MaritalStatus::Married;
    a.spouse = Some(Spouse {
        is_canadian_citizen_or_pr: false,
        accompanying: true,
        education: Some(EducationLevel::BachelorOrThreeYear),
        canadian_work_years: 1,
        language: Some(lt(TestKind::CelpipGeneral, [7, 7, 7, 7])),
    });
    a.age = 35;
    a.education = EducationLevel::MasterOrProfessional;
    a.first_language = Some(lt(TestKind::TefCanada, [470, 440, 440, 450])); // post-2023 scale: NCLC 7
    a.second_language = Some(lt(TestKind::IeltsGeneral, [60, 60, 60, 60])); // CLB 7
    a.canadian_work_years = 0;
    a.foreign_work_years = 2;
    a.canadian_education = Some(CanadianEducation::ThreeYearOrMore);
    a.sibling_in_canada = true;
    let r = score(&load(V0), &a, &d(TODAY)).unwrap();
    assert!(r.with_spouse);
    assert_eq!(
        (
            r.core.subtotal,
            r.spouse.subtotal,
            r.transferability.subtotal,
            r.additional.subtotal,
            r.total
        ),
        (272, 25, 38, 95, 430)
    );
    assert_eq!(r.additional.line("French language skills"), Some(50));

    // Spouse who is a citizen: scored as single, spouse factors ignored.
    a.spouse.as_mut().unwrap().is_canadian_citizen_or_pr = true;
    let r = score(&load(V0), &a, &d(TODAY)).unwrap();
    assert!(!r.with_spouse);
    assert_eq!(r.spouse.subtotal, 0);
}

#[test]
fn french_bonus_tiers() {
    let mut a = base();
    a.first_language = Some(lt(TestKind::TefCanada, [470, 440, 440, 450])); // NCLC 7
    let r = score(&load(V0), &a, &d(TODAY)).unwrap();
    assert_eq!(
        r.additional.line("French language skills"),
        Some(25),
        "no English test"
    );
    a.second_language = Some(lt(TestKind::IeltsGeneral, [40, 45, 35, 40])); // CLB 4
    let r = score(&load(V0), &a, &d(TODAY)).unwrap();
    assert_eq!(
        r.additional.line("French language skills"),
        Some(25),
        "English CLB 4"
    );
    a.second_language = Some(lt(TestKind::IeltsGeneral, [50, 50, 40, 50])); // CLB 5
    let r = score(&load(V0), &a, &d(TODAY)).unwrap();
    assert_eq!(
        r.additional.line("French language skills"),
        Some(50),
        "English CLB 5"
    );
    a.first_language = Some(lt(TestKind::TefCanada, [430, 440, 440, 450])); // speaking NCLC 6
    let r = score(&load(V0), &a, &d(TODAY)).unwrap();
    assert_eq!(r.additional.line("French language skills"), None);
}

#[test]
fn caps() {
    // Nomination 600 + sibling 15 + Canadian education 30 → additional capped at 600.
    let mut a = base();
    a.provincial_nomination = true;
    a.sibling_in_canada = true;
    a.canadian_education = Some(CanadianEducation::ThreeYearOrMore);
    let r = score(&load(V0), &a, &d(TODAY)).unwrap();
    assert_eq!(r.additional.subtotal, 600);

    // Transferability: education 50 + foreign 50 + certificate 50 → capped at 100.
    let mut a = base();
    a.education = EducationLevel::Doctorate;
    a.canadian_work_years = 2;
    a.certificate_of_qualification = true;
    let r = score(&load(V0), &a, &d(TODAY)).unwrap();
    assert_eq!(r.transferability.subtotal, 100);
}

#[test]
fn labour_market_from_2026_11_03() {
    // Single, 30, bachelor, CELPIP 10 (CLB 10), 3 yrs Canada in NOC 00011 (2× tier).
    let mut a = base();
    a.age = 30;
    a.first_language = Some(lt(TestKind::CelpipGeneral, [10, 10, 10, 10]));
    a.canadian_work_years = 3;
    a.canadian_work_by_noc = vec![NocMonths {
        noc: Noc::parse("00011").unwrap(),
        months: 36,
    }];
    a.foreign_work_years = 0;

    // Before: core 105 + 120 + 34 × 4 + 64 = 425; transfer 25 + 25 = 50 → 475.
    let r = score(&load(V0), &a, &d(TODAY)).unwrap();
    assert_eq!(
        (r.core.subtotal, r.transferability.subtotal, r.total),
        (425, 50, 475)
    );
    assert!(r.labour_market.is_none());

    // After: Canadian work leaves the core (361); labour market = 64 + 70 = 134 → 545.
    let r = score(&load(V1), &a, &d("2026-11-03")).unwrap();
    assert_eq!(r.core.line("Canadian work experience"), None);
    assert_eq!(r.core.subtotal, 361);
    assert_eq!(r.labour_market.as_ref().unwrap().subtotal, 134);
    assert_eq!(r.total, 545);

    // 11 months in a high-wage job is not enough for the bonus.
    a.canadian_work_years = 1;
    a.canadian_work_by_noc = vec![
        NocMonths {
            noc: Noc::parse("00011").unwrap(),
            months: 11,
        },
        NocMonths {
            noc: Noc::parse("64100").unwrap(),
            months: 2,
        },
    ];
    let r = score(&load(V1), &a, &d("2026-11-03")).unwrap();
    assert_eq!(r.labour_market.unwrap().subtotal, 40, "duration only");
}

#[test]
fn job_offer_counts_only_from_2026_11_03() {
    let mut a = base();
    a.canadian_work_years = 0;
    a.job_offer = Some(JobOffer {
        noc: Noc::parse("00011").unwrap(),
    });
    let before = score(&load(V0), &a, &d(TODAY)).unwrap();
    let after = score(&load(V1), &a, &d("2026-11-03")).unwrap();
    let lm = after.labour_market.unwrap();
    assert_eq!(lm.line("qualifying job offer"), Some(100));
    // Canadian work points left the core but were 0 here, so the difference is the offer.
    assert_eq!(after.total, before.total + 100);

    // A job offer outside the high-wage list earns nothing.
    a.job_offer = Some(JobOffer {
        noc: Noc::parse("64100").unwrap(),
    });
    let r = score(&load(V1), &a, &d("2026-11-03")).unwrap();
    assert_eq!(r.labour_market.unwrap().subtotal, 0);
}

#[test]
fn missing_breakdown_is_refused_after_2026_11_03() {
    let e = score(&load(V1), &base(), &d("2026-11-03")).unwrap_err();
    assert!(matches!(e, CrsError::Missing(_)), "{e:?}");
}

#[test]
fn divergence_flags() {
    // D-001: TCF first language with listening at NCLC 4 → 6 points, flagged.
    let mut a = base();
    a.first_language = Some(lt(TestKind::TcfCanada, [12, 340, 500, 12]));
    let r = score(&load(V0), &a, &d(TODAY)).unwrap();
    assert!(
        r.flags
            .contains(&Flag::KnownDivergence { id: "D-001".into() })
    );
    // Without spouse: NCLC 8, 4, 8, 8 → 23 + 6 + 23 + 23.
    assert_eq!(r.core.line("first official language"), Some(75));

    // D-002: second language at NCLC 4 → 0, flagged via the ruling citation.
    let mut a = base();
    a.second_language = Some(lt(TestKind::TefCanada, [330, 310, 310, 270]));
    let r = score(&load(V0), &a, &d(TODAY)).unwrap();
    assert_eq!(r.core.line("second official language"), Some(0));
    assert!(
        r.flags
            .contains(&Flag::KnownDivergence { id: "D-002".into() })
    );
}

#[test]
fn refusals() {
    let mut a = base();
    a.first_language = None;
    assert_eq!(
        score(&load(V0), &a, &d(TODAY)).unwrap_err(),
        CrsError::NoFirstLanguage
    );

    // A grid without the D-002 ruling is refused.
    let raw: Grid = serde_json::from_str(V0).unwrap();
    assert!(matches!(
        score(&raw, &base(), &d(TODAY)).unwrap_err(),
        CrsError::GridIssues(..)
    ));

    let mut a = base();
    a.age = 18;
    a.canadian_work_years = 5;
    assert!(matches!(
        score(&load(V0), &a, &d(TODAY)).unwrap_err(),
        CrsError::Invalid(_)
    ));
}

#[test]
fn expired_test_is_flagged() {
    let mut a = base();
    a.first_language.as_mut().unwrap().test_date = d("2024-01-01");
    let r = score(&load(V0), &a, &d(TODAY)).unwrap();
    assert!(r.flags.contains(&Flag::ExpiredLanguageTest {
        which: "first language"
    }));
}

#[test]
fn d004_french_first_with_pte_english() {
    let mut a = base();
    a.first_language = Some(lt(TestKind::TefCanada, [470, 440, 440, 450])); // NCLC 7
    a.second_language = Some(lt(TestKind::PteCore, [51, 39, 42, 51])); // CLB 5
    let r = score(&load(V0), &a, &d(TODAY)).unwrap();
    assert_eq!(r.additional.line("French language skills"), Some(50));
    assert!(
        r.flags
            .contains(&Flag::KnownDivergence { id: "D-004".into() })
    );
}

/// Every divergence a score relies on comes with a blocking review flag.
#[test]
fn every_divergence_requires_review() {
    let ids = |r: &rules_engine::crs::CrsResult| -> (Vec<String>, Vec<String>) {
        let mut known = vec![];
        let mut review = vec![];
        for f in &r.flags {
            match f {
                Flag::KnownDivergence { id } => known.push(id.clone()),
                Flag::ManualReview { id, reason } => {
                    assert!(!reason.is_empty());
                    review.push(id.clone());
                }
                _ => {}
            }
        }
        (known, review)
    };
    let g = load(V0);

    // D-001 (TCF NCLC 4) and D-003 never apply to TCF.
    let mut a = base();
    a.first_language = Some(lt(TestKind::TcfCanada, [12, 340, 500, 12]));
    let (k, r) = ids(&score(&g, &a, &d(TODAY)).unwrap());
    assert_eq!((k, r), (vec!["D-001".to_owned()], vec!["D-001".to_owned()]));

    // D-002 (second language NCLC 4) + D-003 (TEF on the post-2023 scale).
    let mut a = base();
    a.second_language = Some(lt(TestKind::TefCanada, [330, 310, 310, 270]));
    let (mut k, mut r) = ids(&score(&g, &a, &d(TODAY)).unwrap());
    k.sort();
    r.sort();
    assert_eq!(k, vec!["D-002", "D-003"]);
    assert_eq!(r, k);

    // TEF on the pre-2019 scale matches the Express Entry pages: no D-003.
    let mut a = base();
    a.second_language = Some(LanguageTest {
        test_date: d("2019-01-15"),
        ..lt(TestKind::TefCanada, [350, 290, 240, 350])
    });
    let res = score(&g, &a, &d(TODAY)).unwrap();
    assert!(
        !res.flags
            .iter()
            .any(|f| matches!(f, Flag::KnownDivergence { id } if id == "D-003"))
    );

    // No divergence: no review flag at all.
    assert!(
        !score(&g, &base(), &d(TODAY))
            .unwrap()
            .flags
            .iter()
            .any(Flag::blocks_approval)
    );
}
