//! Bridge to the official calculator run headless (`tools/oracle/oracle.mjs`).
//!
//! [`answers`] maps an [`Applicant`] onto the calculator's form fields;
//! [`Oracle`] keeps one Node process alive and scores profiles through it.

use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use applicant::{
    Applicant, CanadianEducation, EducationLevel, JobOfferNoc, LanguageTest, MaritalStatus,
    OfficialLanguage, TestKind,
};
use rules_engine::grid::Grid;
use rules_engine::language::{Clb, to_clb};
use serde::Deserialize;

/// The calculator's result sections.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct Sections {
    pub core: u16,
    pub spouse: u16,
    pub skill: u16,
    pub bonus: u16,
    pub total: u16,
}

#[derive(Debug, Deserialize)]
struct Reply {
    #[serde(flatten)]
    sections: Option<Sections>,
    error: Option<String>,
}

fn yes_no(b: bool) -> &'static str {
    if b { "B" } else { "A" }
}

fn marital(m: MaritalStatus) -> &'static str {
    use MaritalStatus::*;
    match m {
        Annulled => "A",
        CommonLaw => "B",
        Divorced => "C",
        LegallySeparated => "D",
        Married => "E",
        NeverMarried => "F",
        Widowed => "G",
    }
}

fn age(a: u8) -> String {
    const CODES: [&str; 29] = [
        "A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N", "O", "P", "Q", "R",
        "S", "T", "U", "V", "W", "X", "Y", "Z", "AA", "AB", "AC",
    ];
    CODES[usize::from(a.clamp(17, 45) - 17)].to_owned()
}

fn education(e: EducationLevel) -> &'static str {
    use EducationLevel::*;
    match e {
        LessThanSecondary => "A",
        Secondary => "B",
        OneYearPostSecondary => "C",
        TwoYearPostSecondary => "D",
        BachelorOrThreeYear => "E",
        TwoOrMoreCredentials => "F",
        MasterOrProfessional => "G",
        Doctorate => "H",
    }
}

/// q5i-a / q12i test codes.
fn test_code(t: TestKind) -> &'static str {
    match t {
        TestKind::CelpipGeneral => "A",
        TestKind::IeltsGeneral => "B",
        TestKind::TefCanada => "C",
        TestKind::TcfCanada => "D",
        TestKind::PteCore => "E",
    }
}

/// Calculator option row for a CLB level (rows A..H = below 4, 4..10+).
fn level(clb: u8) -> &'static str {
    match clb {
        10.. => "H",
        9 => "G",
        8 => "F",
        7 => "E",
        6 => "D",
        5 => "C",
        4 => "B",
        _ => "A",
    }
}

fn years(y: u8, max_code: u8) -> String {
    char::from(b'A' + y.min(max_code)).to_string()
}

fn skills(prefix: &str, c: &Clb, out: &mut Vec<(String, String)>) {
    for (name, v) in [
        ("speaking", c.speaking),
        ("listening", c.listening),
        ("reading", c.reading),
        ("writing", c.writing),
    ] {
        out.push((format!("{prefix}-{name}"), level(v).to_owned()));
    }
}

/// Form answers for `a`, in fill-in order. `None` if the calculator cannot
/// express the profile (no first-language test, or a conversion error).
pub fn answers(grid: &Grid, a: &Applicant) -> Option<Vec<(String, String)>> {
    let clb = |t: &LanguageTest| to_clb(grid, t).ok().map(|(c, _)| c);
    let fol_test = a.first_language.as_ref()?;
    let fol = clb(fol_test)?;
    let mut out: Vec<(String, String)> = Vec::new();
    let mut put = |k: &str, v: &str| out.push((k.to_owned(), v.to_owned()));

    put("q1", marital(a.marital_status));
    if let Some(s) = a.spouse.as_ref().filter(|_| a.marital_status.has_partner()) {
        put("q2i", yes_no(s.is_canadian_citizen_or_pr));
        if !s.is_canadian_citizen_or_pr {
            put("q2ii", yes_no(s.accompanying));
        }
    }
    put("q3", &age(a.age));
    put("q4", education(a.education));
    put("q4b", yes_no(a.canadian_education.is_some()));
    if let Some(c) = a.canadian_education {
        put(
            "q4c",
            match c {
                CanadianEducation::SecondaryOrLess => "A",
                CanadianEducation::OneOrTwoYear => "B",
                CanadianEducation::ThreeYearOrMore => "C",
            },
        );
    }
    put("q5i", "A");
    put("q5i-a", test_code(fol_test.test));
    skills("q5i-b", &fol, &mut out);

    let english_first = fol_test.test.language() == OfficialLanguage::English;
    let sol_code = match a.second_language.as_ref().map(|t| t.test) {
        None => "C",
        Some(TestKind::TefCanada) if english_first => "A",
        Some(TestKind::TcfCanada) if english_first => "B",
        Some(TestKind::CelpipGeneral) => "A",
        Some(TestKind::IeltsGeneral) => "B",
        Some(TestKind::PteCore) => "E",
        Some(_) => return None,
    };
    out.push(("q5ii".into(), sol_code.into()));
    if let Some(t) = &a.second_language {
        skills("q5ii-sol", &clb(t)?, &mut out);
    }

    let mut put = |k: &str, v: &str| out.push((k.to_owned(), v.to_owned()));
    put("q6i", &years(a.canadian_work_years, 5));
    put("q6ii", &years(a.foreign_work_years, 3));
    put("q7", yes_no(a.certificate_of_qualification));
    put("q8", yes_no(a.job_offer.is_some()));
    if let Some(o) = &a.job_offer {
        put(
            "q8a",
            match o.noc.offer_category() {
                JobOfferNoc::Teer0Major00 => "A",
                JobOfferNoc::Teer0123 => "B",
                JobOfferNoc::Teer45 => "C",
            },
        );
    }
    put("q9", yes_no(a.provincial_nomination));
    put("q10i", yes_no(a.sibling_in_canada));

    if a.scored_with_spouse() {
        let s = a.spouse.as_ref()?;
        put("q10", education(s.education?));
        put("q11", &years(s.canadian_work_years, 5));
        match &s.language {
            None => put("q12i", "F"),
            Some(t) => {
                put("q12i", test_code(t.test));
                skills("q12ii-fol", &clb(t)?, &mut out);
            }
        }
    }
    Some(out)
}

pub struct Oracle {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next: u64,
}

impl Oracle {
    /// Starts `node tools/oracle/oracle.mjs`. Requires `npm ci` in tools/oracle.
    pub fn start() -> std::io::Result<Self> {
        let script =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tools/oracle/oracle.mjs");
        let mut child = Command::new(std::env::var("NODE").unwrap_or_else(|_| "node".into()))
            .arg(script)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| std::io::Error::other("no stdin"))?;
        let stdout = BufReader::new(
            child
                .stdout
                .take()
                .ok_or_else(|| std::io::Error::other("no stdout"))?,
        );
        Ok(Self {
            child,
            stdin,
            stdout,
            next: 0,
        })
    }

    pub fn score(&mut self, answers: &[(String, String)]) -> Result<Sections, String> {
        self.next += 1;
        let req = serde_json::json!({ "id": self.next.to_string(), "answers": answers });
        writeln!(self.stdin, "{req}").map_err(|e| e.to_string())?;
        self.stdin.flush().map_err(|e| e.to_string())?;
        let mut line = String::new();
        self.stdout
            .read_line(&mut line)
            .map_err(|e| e.to_string())?;
        let reply: Reply = serde_json::from_str(&line).map_err(|e| format!("{e}: {line}"))?;
        match (reply.sections, reply.error) {
            (_, Some(e)) => Err(e),
            (Some(s), None) => Ok(s),
            (None, None) => Err(format!("empty reply: {line}")),
        }
    }
}

impl Drop for Oracle {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
