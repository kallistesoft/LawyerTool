#![allow(clippy::unwrap_used, clippy::expect_used)] // test code may panic

//! Differential fuzzing: rules-engine vs the official IRCC calculator.
//!
//! Needs Node and `npm ci` in `tools/oracle`, so it is `#[ignore]` by default:
//!   cargo test -p oracle-tests -- --include-ignored
//! Case count: `PROPTEST_CASES` (default 5000). Failures are shrunk to a
//! minimal profile and its seed saved in `differential.proptest-regressions`
//! next to this file (commit it: CI replays every saved seed).
//!
//! Known divergences are adjusted for, never skipped:
//! - D-001: calculator gives 0 (we give 6) per TCF first-language skill at NCLC 4.
//! - D-004: calculator gives 25 (we give 50) French bonus when French is first
//!   and the English second-language test is PTE Core at CLB 5+.

use std::cell::RefCell;
use std::sync::LazyLock;

use applicant::*;
use oracle_tests::{Oracle, Sections, answers};
use proptest::prelude::*;
use rules_engine::crs::score;
use rules_engine::grid::Grid;
use rules_engine::language::{table, to_clb};

static GRID: LazyLock<Grid> = LazyLock::new(|| {
    let mut g: Grid =
        serde_json::from_str(include_str!("../../rules-engine/grids/crs-2026-10-16.json")).unwrap();
    rules_engine::rulings::apply(&mut g);
    g
});

const ASSESSED: &str = "2026-10-18";
const TEST_DATE: &str = "2025-06-01";

thread_local! {
    static ORACLE: RefCell<Oracle> = RefCell::new(Oracle::start().expect("start node oracle (npm ci in tools/oracle?)"));
}

const TESTS: [TestKind; 5] = [
    TestKind::CelpipGeneral,
    TestKind::IeltsGeneral,
    TestKind::PteCore,
    TestKind::TefCanada,
    TestKind::TcfCanada,
];

/// A raw score that converts to `clb` (3 = below the table) for this test.
fn score_for(test: TestKind, skill: Skill, clb: u8, pick: u16) -> u16 {
    let t = table(&GRID, test, &IsoDate::parse(TEST_DATE).unwrap()).unwrap();
    let (lo, hi, step) = test.score_range(skill);
    let lowest = t.rows[0].clb;
    let (min, max) = if clb < lowest {
        (lo, t.rows[0].band(skill).min.saturating_sub(step).max(lo))
    } else {
        let i = t.rows.iter().position(|r| r.clb == clb).unwrap();
        let b = t.rows[i].band(skill);
        let next = t.rows.get(i + 1).map(|r| r.band(skill).min - step);
        (b.min, b.max.or(next).unwrap_or(hi).min(hi))
    };
    let span = (max - min) / step + 1;
    min + (pick % span) * step
}

fn language(test: impl Strategy<Value = TestKind>) -> impl Strategy<Value = LanguageTest> {
    (
        test,
        prop::array::uniform4(3u8..=10),
        prop::array::uniform4(any::<u16>()),
    )
        .prop_map(|(test, clb, pick)| {
            let s = |i: usize| score_for(test, Skill::ALL[i], clb[i], pick[i]);
            LanguageTest {
                test,
                test_date: IsoDate::parse(TEST_DATE).unwrap(),
                speaking: s(0),
                listening: s(1),
                reading: s(2),
                writing: s(3),
            }
        })
}

fn education() -> impl Strategy<Value = EducationLevel> {
    use EducationLevel::*;
    prop_oneof![
        Just(LessThanSecondary),
        Just(Secondary),
        Just(OneYearPostSecondary),
        Just(TwoYearPostSecondary),
        Just(BachelorOrThreeYear),
        Just(TwoOrMoreCredentials),
        Just(MasterOrProfessional),
        Just(Doctorate),
    ]
}

fn marital() -> impl Strategy<Value = MaritalStatus> {
    use MaritalStatus::*;
    prop_oneof![
        Just(Annulled),
        Just(CommonLaw),
        Just(Divorced),
        Just(LegallySeparated),
        Just(Married),
        Just(NeverMarried),
        Just(Widowed),
    ]
}

fn applicant() -> impl Strategy<Value = Applicant> {
    let fol_test = prop::sample::select(TESTS.to_vec());
    let spouse = (
        any::<bool>(),
        any::<bool>(),
        education(),
        0u8..=6,
        prop::option::of(language(prop::sample::select(TESTS.to_vec()))),
    )
        .prop_map(|(cit, acc, edu, years, language)| Spouse {
            is_canadian_citizen_or_pr: cit,
            accompanying: acc,
            education: Some(edu),
            canadian_work_years: years,
            language,
        });
    (
        (marital(), spouse, 17u8..=50, education()),
        (
            prop::option::of(prop_oneof![
                Just(CanadianEducation::SecondaryOrLess),
                Just(CanadianEducation::OneOrTwoYear),
                Just(CanadianEducation::ThreeYearOrMore),
            ]),
            language(fol_test).prop_flat_map(|fol| {
                let other: Vec<TestKind> = TESTS
                    .iter()
                    .copied()
                    .filter(|t| t.language() != fol.test.language())
                    .collect();
                (
                    Just(fol),
                    prop::option::of(language(prop::sample::select(other))),
                )
            }),
        ),
        (
            0u8..=6,
            0u8..=4,
            any::<bool>(),
            prop::option::weighted(0.2, prop::sample::select(vec!["00012", "21231", "64100"])),
        ),
        (prop::bool::weighted(0.1), any::<bool>()),
    )
        .prop_map(
            |(
                (marital_status, spouse, age, education),
                (canadian_education, (fol, sol)),
                (cw, fw, cert, offer),
                (pnp, sib),
            )| {
                Applicant {
                    marital_status,
                    spouse: marital_status.has_partner().then_some(spouse),
                    age,
                    education,
                    canadian_education,
                    first_language: Some(fol),
                    second_language: sol,
                    canadian_work_years: cw,
                    canadian_work_by_noc: vec![],
                    foreign_work_years: fw,
                    certificate_of_qualification: cert,
                    job_offer: offer.map(|n| JobOffer {
                        noc: Noc::parse(n).unwrap(),
                    }),
                    provincial_nomination: pnp,
                    sibling_in_canada: sib,
                    settlement_funds_cad: None,
                    family_size: None,
                }
            },
        )
        .prop_filter("valid profile", |a| a.validate().is_ok())
}

fn engine(a: &Applicant) -> Sections {
    let r = score(&GRID, a, &IsoDate::parse(ASSESSED).unwrap()).unwrap();
    Sections {
        core: r.core.subtotal,
        spouse: r.spouse.subtotal,
        skill: r.transferability.subtotal,
        bonus: r.additional.subtotal,
        total: r.total,
    }
}

/// Calculator result with known divergences corrected to our ruling.
fn oracle(a: &Applicant) -> Sections {
    let form = answers(&GRID, a).expect("profile is expressible in the calculator");
    let mut s = ORACLE
        .with(|o| o.borrow_mut().score(&form))
        .unwrap_or_else(|e| panic!("oracle error: {e}\nform: {form:?}"));
    let fol = a.first_language.as_ref().unwrap();
    if fol.test == TestKind::TcfCanada {
        let (clb, _) = to_clb(&GRID, fol).unwrap();
        let n = Skill::ALL.iter().filter(|sk| clb.get(**sk) == 4).count() as u16;
        s.core += 6 * n; // D-001
        s.total += 6 * n;
    }
    if let Some(sol) = a
        .second_language
        .as_ref()
        .filter(|t| t.test == TestKind::PteCore)
    {
        let french = to_clb(&GRID, fol).unwrap().0;
        let english = to_clb(&GRID, sol).unwrap().0;
        if fol.test.language() == OfficialLanguage::French
            && french.all_at_least(7)
            && english.all_at_least(5)
        {
            let bonus = (s.bonus + 25).min(600); // D-004
            s.total += bonus - s.bonus;
            s.bonus = bonus;
        }
    }
    s
}

fn cases() -> u32 {
    std::env::var("PROPTEST_CASES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(5000)
}

proptest! {
    #![proptest_config(ProptestConfig { cases: cases(), max_shrink_iters: 2000, ..ProptestConfig::default() })]

    #[test]
    #[ignore = "needs node + npm ci in tools/oracle"]
    fn engine_matches_official_calculator(a in applicant()) {
        prop_assert_eq!(engine(&a), oracle(&a), "profile: {:#?}", a);
    }
}

/// Harness self-test on the hand-computed profile from rules-engine/tests/crs.rs.
#[test]
#[ignore = "needs node + npm ci in tools/oracle"]
fn harness_reproduces_known_profile() {
    let a = Applicant {
        marital_status: MaritalStatus::NeverMarried,
        spouse: None,
        age: 29,
        education: EducationLevel::BachelorOrThreeYear,
        canadian_education: None,
        first_language: Some(LanguageTest {
            test: TestKind::IeltsGeneral,
            test_date: IsoDate::parse(TEST_DATE).unwrap(),
            speaking: 70,
            listening: 80,
            reading: 70,
            writing: 70,
        }),
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
    };
    assert_eq!(
        oracle(&a),
        Sections {
            core: 394,
            spouse: 0,
            skill: 88,
            bonus: 0,
            total: 482
        }
    );
    assert_eq!(engine(&a), oracle(&a));
}
