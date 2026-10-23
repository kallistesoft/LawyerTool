//! What the audit needs to know about a respondent, read from the form's
//! answers and the lawyer's inputs (CNP codes, regulated profession).

use std::collections::BTreeMap;

use form_import::{funds, is_no, is_yes, leading_number, noc_in, norm};
use serde::{Deserialize, Serialize};

/// What the lawyer sets in the app before generating the audit.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lawyer {
    /// Applicant's CNP (5 digits) and whether the lawyer validated it.
    pub noc: Option<String>,
    #[serde(default)]
    pub noc_validated: bool,
    pub spouse_noc: Option<String>,
    #[serde(default)]
    pub spouse_noc_validated: bool,
    /// End of « Votre profession … » in the roadmap (e.g. « n'est pas
    /// réglementée », « est réglementée en Alberta »). `None` = to check.
    pub regulated: Option<String>,
    /// Free paragraph added to the bilan.
    pub note: Option<String>,
}

/// The form answers kept with an imported client (vault JSON).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FormRecord {
    /// Answers by `form_import::Q::key`.
    pub answers: BTreeMap<String, String>,
    pub age: Option<u8>,
    #[serde(default)]
    pub lawyer: Lawyer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Beginner,
    Intermediate,
    Bilingual,
    Native,
}

impl Level {
    fn of(answer: &str) -> Option<Self> {
        let n = norm(answer);
        if n.starts_with("langue maternelle") {
            Some(Self::Native)
        } else if n.starts_with("bilingue") {
            Some(Self::Bilingual)
        } else if n.starts_with("intermediaire") {
            Some(Self::Intermediate)
        } else if n.starts_with("debutant") {
            Some(Self::Beginner)
        } else {
            None
        }
    }

    pub fn fr(self) -> &'static str {
        match self {
            Self::Beginner => "débutant",
            Self::Intermediate => "intermédiaire",
            Self::Bilingual => "bilingue",
            Self::Native => "langue maternelle",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Funds {
    /// Household size and declared savings (CAD).
    Enough(u8, u32),
    NotEnough,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrenchTest {
    Done,
    InProgress,
    ToTake,
}

/// A CNP code with its FEER, and whether it is only the client's own code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Noc {
    pub code: String,
    pub feer: u8,
    pub validated: bool,
}

impl Noc {
    fn of(code: Option<&str>, validated: bool) -> Option<Self> {
        let code = code?.trim();
        let ok = code.len() == 5 && code.bytes().all(|b| b.is_ascii_digit());
        ok.then(|| Self {
            code: code.to_owned(),
            feer: code.as_bytes()[1] - b'0',
            validated,
        })
    }

    /// FEER 0 to 3: Express Entry and spouse's open work permit possible.
    pub fn qualified(&self) -> bool {
        self.feer <= 3
    }
}

#[derive(Debug, Clone)]
pub struct Facts {
    pub answers: BTreeMap<String, String>,
    pub age: Option<u8>,
    pub married: bool,
    pub spouse_age: Option<u8>,
    pub french: Option<Level>,
    pub english: Option<Level>,
    pub spouse_english: Option<Level>,
    pub french_test: FrenchTest,
    pub spouse_test: bool,
    pub eca_started: bool,
    pub diploma_in_progress: bool,
    pub spouse_has_bac: Option<bool>,
    pub funds: Funds,
    pub quebec: bool,
    pub quebec_only: bool,
    pub alberta: bool,
    pub nb: bool,
    pub relative: bool,
    pub noc: Option<Noc>,
    pub spouse_noc: Option<Noc>,
    pub lawyer: Lawyer,
}

/// Other destinations than Québec named in a province answer.
const ELSEWHERE: &[&str] = &[
    "alberta",
    "ontario",
    "colombie",
    "british",
    "bc",
    "vancouver",
    "victoria",
    "brunswick",
    "nb",
    "manitoba",
    "saskatchewan",
    "ecosse",
    "scotia",
    "ile-du",
    "edouard",
    "terre-neuve",
    "yukon",
    "nunavut",
    "territoires",
    "toronto",
    "ottawa",
    "calgary",
    "edmonton",
    "winnipeg",
    "halifax",
    "moncton",
    "autre",
    "autres",
    "partout",
    "anglophone",
    "anglophones",
    "canada",
];

fn words(s: &str) -> Vec<String> {
    norm(s)
        .split(|c: char| !(c.is_alphanumeric() || c == '-'))
        .filter(|w| !w.is_empty())
        .map(str::to_owned)
        .collect()
}

/// First whole number between 16 and 99 in a short answer ("36 ans").
fn first_age(s: &str) -> Option<u8> {
    s.split(|c: char| !c.is_ascii_digit())
        .filter(|w| (1..=2).contains(&w.len()))
        .filter_map(|w| w.parse::<u8>().ok())
        .find(|a| (16..=99).contains(a))
}

impl Facts {
    pub fn of(f: &FormRecord) -> Self {
        let a = |k: &str| f.answers.get(k).map(String::as_str).unwrap_or("");
        let marital = norm(a("marital"));
        let province = norm(a("province"));
        let pw = words(a("province"));
        let quebec = province.contains("quebec") || province.contains("montreal");
        let elsewhere = pw.iter().any(|w| ELSEWHERE.contains(&w.as_str()));
        let french_test = match norm(a("french_test")).as_str() {
            t if t.starts_with("oui") => FrenchTest::Done,
            t if t.starts_with("en cours") => FrenchTest::InProgress,
            _ => FrenchTest::ToTake,
        };
        let funds_answer = a("funds");
        let funds = if funds_answer.is_empty() {
            Funds::Unknown
        } else if norm(funds_answer).contains("pas les fonds") {
            Funds::NotEnough
        } else {
            funds(funds_answer).map_or(Funds::Unknown, |(n, x)| Funds::Enough(n, x))
        };
        let spouse_education = a("spouse_education");
        let lawyer = f.lawyer.clone();
        let noc = Noc::of(lawyer.noc.as_deref(), lawyer.noc_validated)
            .or_else(|| Noc::of(noc_in(a("job")).as_deref(), false));
        let spouse_noc = Noc::of(lawyer.spouse_noc.as_deref(), lawyer.spouse_noc_validated)
            .or_else(|| Noc::of(noc_in(a("spouse_job")).as_deref(), false));
        let diploma = norm(a("education"));
        Self {
            age: f.age,
            married: marital.contains("marie") || marital.contains("union"),
            spouse_age: first_age(a("spouse_age")),
            french: Level::of(a("french")),
            english: Level::of(a("english")),
            spouse_english: Level::of(a("spouse_english")),
            french_test,
            spouse_test: is_yes(a("spouse_test")),
            eca_started: norm(a("eca")).contains("faite ou en cours"),
            diploma_in_progress: diploma.contains("en cours") || diploma.contains("vae"),
            spouse_has_bac: (!spouse_education.is_empty())
                .then(|| form_import::assumed_spouse_education(spouse_education))
                .map(|e| e != applicant::EducationLevel::LessThanSecondary),
            funds,
            quebec,
            quebec_only: quebec && !elsewhere,
            alberta: province.contains("alberta")
                || province.contains("calgary")
                || province.contains("edmonton"),
            nb: province.contains("brunswick") || pw.iter().any(|w| w == "nb"),
            relative: is_yes(a("relative")),
            noc,
            spouse_noc,
            lawyer,
            answers: f.answers.clone(),
        }
    }

    /// An answer, trimmed (empty if not given).
    pub fn a(&self, key: &str) -> &str {
        self.answers.get(key).map_or("", |s| s.trim())
    }

    /// FEER 4–5: not eligible for Express Entry yet.
    pub fn low_skilled(&self) -> bool {
        self.noc.as_ref().is_some_and(|n| !n.qualified())
    }

    pub fn under_36(&self) -> bool {
        self.age.is_some_and(|a| a <= 35)
    }

    /// French strong enough for the francophone routes (C16, French draws).
    pub fn francophone(&self) -> bool {
        self.french.is_some_and(|l| l >= Level::Bilingual)
    }

    pub fn seniority(&self) -> String {
        let e = self.a("experience");
        match leading_number(e) {
            Some(n) if norm(e).contains("plus") => format!("+{n} ans d’ancienneté"),
            Some(1) => "depuis 1 an".into(),
            Some(n) => format!("depuis {n} ans"),
            None => String::new(),
        }
    }

    /// Declared conviction, suspicion or illness: for the lawyer to examine.
    pub fn concerns(&self) -> Vec<String> {
        let mut out = Vec::new();
        let c = self.a("criminal");
        if !c.is_empty() && !norm(c).starts_with("jamais") && !is_no(c) {
            out.push(format!("Justice : « {c} »"));
        }
        let h = self.a("health");
        if !h.is_empty() && !is_no(h) {
            out.push(format!("Santé : « {h} »"));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(pairs: &[(&str, &str)]) -> FormRecord {
        FormRecord {
            answers: pairs
                .iter()
                .map(|(k, v)| ((*k).into(), (*v).into()))
                .collect(),
            age: Some(32),
            lawyer: Lawyer::default(),
        }
    }

    #[test]
    fn destinations() {
        for (p, q, only, ab, nb) in [
            ("Québec", true, true, false, false),
            (
                "Notre province de destination souhaitée est le Québec",
                true,
                true,
                false,
                false,
            ),
            ("Montréal ou Orléans ", true, true, false, false),
            (
                "Alberta, nouveau brunswick, Yukon ",
                false,
                false,
                true,
                true,
            ),
            ("Ontario ou Québec", true, false, false, false),
            (
                "Alberta ou New-Brunswick ou Quebec",
                true,
                false,
                true,
                true,
            ),
            ("Québec ou autres provinces", true, false, false, false),
            ("Le nouveau Brunswick", false, false, false, true),
        ] {
            let f = Facts::of(&record(&[("province", p)]));
            assert_eq!(
                (f.quebec, f.quebec_only, f.alberta, f.nb),
                (q, only, ab, nb),
                "{p}"
            );
        }
    }

    #[test]
    fn noc_from_the_lawyer_or_the_client() {
        let mut r = record(&[("job", "Infirmière (code CNP 31301)")]);
        let f = Facts::of(&r);
        assert_eq!(
            f.noc,
            Some(Noc {
                code: "31301".into(),
                feer: 1,
                validated: false
            })
        );
        r.lawyer.noc = Some("64410".into());
        r.lawyer.noc_validated = true;
        let f = Facts::of(&r);
        assert!(f.low_skilled() && f.noc.as_ref().is_some_and(|n| n.validated));
        assert!(Facts::of(&record(&[("job", "Comptable")])).noc.is_none());
    }

    #[test]
    fn funds_and_levels() {
        let f = Facts::of(&record(&[
            (
                "funds",
                "4 personnes : J'ai au moins 30 000 $ CAD d'économies\u{a0}",
            ),
            ("french", "Langue maternelle"),
            ("english", "Intermédiaire\u{a0}"),
            ("french_test", "En cours"),
            ("marital", "Marié ou en union de fait"),
            ("spouse_age", "42 né le 27/11/1983"),
            ("spouse_education", "Il/elle n'a pas le baccalauréat"),
            ("experience", "5 ans et plus"),
        ]));
        assert_eq!(f.funds, Funds::Enough(4, 30_000));
        assert_eq!(
            (f.french, f.english),
            (Some(Level::Native), Some(Level::Intermediate))
        );
        assert_eq!(f.french_test, FrenchTest::InProgress);
        assert!(f.married && f.francophone());
        assert_eq!(f.spouse_age, Some(42));
        assert_eq!(f.spouse_has_bac, Some(false));
        assert_eq!(f.seniority(), "+5 ans d’ancienneté");
        let none = Facts::of(&record(&[("funds", "Je n'ai pas les fonds disponibles.")]));
        assert_eq!(none.funds, Funds::NotEnough);
    }
}
