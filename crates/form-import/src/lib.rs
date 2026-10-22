//! The firm's intake form ("AUDIT de votre profil — IMMIGRATION AU CANADA",
//! Microsoft Forms) exported as `.xlsx` (or `.csv`): one row per response.
//!
//! The form asks for levels and free text, not the official facts the CRS
//! needs (no test scores, education in free text, experience in brackets).
//! So each response becomes a **partial** profile: only answers that map
//! without interpretation are used (ruled 2026-10-21); everything else is
//! left at its most conservative value and listed in `notes` for the lawyer
//! to complete. Deterministic: no Claude.
//!
//! Columns are found by their question text, not their position, so a
//! re-ordered form still imports; a file that is not this form is refused.
//!
//! Each response also carries an [`Estimate`]: the same profile completed
//! with explicit assumptions so a preliminary CRS score can be calculated
//! (`rules_engine::crs::estimate`). Rulings of 2026-10-22: languages use the
//! cautious table ([`assumed_level`]); a "Oui" to the relative question
//! counts as a sibling; the estimate is shown apart from the official score.

mod table;

use applicant::{
    Applicant, Credential, EducationLevel, EligibilityFacts, IsoDate, MaritalStatus, Spouse,
};
use rules_engine::crs::AssumedLevels;
use sha2::{Digest, Sha256};

pub use table::{read_csv, read_xlsx};

/// One response, ready to become a client.
#[derive(Debug, Clone, PartialEq)]
pub struct Response {
    /// Stable id of the response (form row id + start time + name), so the
    /// same response in a later, longer export is not imported twice.
    pub key: String,
    pub full_name: String,
    pub email: Option<String>,
    pub applicant: Applicant,
    /// What the lawyer must complete or check (French).
    pub notes: Vec<String>,
    pub estimate: Estimate,
}

/// The profile completed with assumptions, for a preliminary score only.
#[derive(Debug, Clone, PartialEq)]
pub struct Estimate {
    pub applicant: Applicant,
    pub levels: AssumedLevels,
    /// Every assumption made, in French, quoting the client's answer.
    pub assumptions: Vec<String>,
    /// Answers that can stop an application on their own (criminality,
    /// no usable language level), whatever the score.
    pub stoppers: Vec<String>,
    /// Answers the lawyer must examine before any verdict (health,
    /// suspicion, Québec, job offer…).
    pub checks: Vec<String>,
    /// The client declared a Canadian job offer (its NOC is unknown).
    pub job_offer_declared: bool,
}

/// CLB/NCLC level assumed for a self-declared language level, in all four
/// skills: the cautious table ruled on 2026-10-22. `None` = no points.
pub fn assumed_level(answer: &str) -> Option<u8> {
    let n = norm(answer);
    if n.starts_with("langue maternelle") {
        Some(9)
    } else if n.starts_with("bilingue") {
        Some(8)
    } else if n.starts_with("intermediaire") {
        Some(6)
    } else {
        None
    }
}

/// Highest diploma (free text) -> education level: the **lowest** level
/// mentioned wins, so "bac+2, master en cours" counts as two years.
pub fn assumed_education(answer: &str) -> Option<EducationLevel> {
    use EducationLevel::*;
    let n = norm(answer)
        .replace(" + ", "+")
        .replace(" +", "+")
        .replace("+ ", "+");
    let words: Vec<&str> = n
        .split(|c: char| !(c.is_alphanumeric() || c == '+'))
        .filter(|w| !w.is_empty())
        .collect();
    let has = |ws: &[&str]| words.iter().any(|w| ws.contains(w));
    [
        (
            Secondary,
            &[
                "bac",
                "baccalaureat",
                "cap",
                "bep",
                "brevet",
                "cess",
                "bac+1",
            ][..],
        ),
        (TwoYearPostSecondary, &["bts", "dut", "deug", "bac+2"][..]),
        (
            BachelorOrThreeYear,
            &["licence", "bachelor", "bac+3", "bac+4"][..],
        ),
        (
            MasterOrProfessional,
            &[
                "master",
                "masters",
                "maitrise",
                "mba",
                "bac+5",
                "bac+6",
                "ingenieur",
            ][..],
        ),
        (Doctorate, &["doctorat", "phd", "bac+8"][..]),
    ]
    .into_iter()
    .find(|(_, ws)| has(ws))
    .map(|(level, _)| level)
}

/// Spouse's diploma (a choice in the form) -> education level (cautious:
/// "4 années d'université et +" counts as three years or more, not master).
pub fn assumed_spouse_education(answer: &str) -> EducationLevel {
    use EducationLevel::*;
    let n = norm(answer);
    if n.contains("n'a pas le baccalaureat") {
        LessThanSecondary
    } else if n.contains("brevet professionnel") || n.contains("le baccalaureat") {
        Secondary
    } else if n.contains("bts") || n.starts_with("2 annees") {
        TwoYearPostSecondary
    } else if n.starts_with("3 annees") || n.starts_with("4 annees") {
        BachelorOrThreeYear
    } else {
        LessThanSecondary
    }
}

fn level_text(level: Option<u8>, scale: &str) -> String {
    level.map_or_else(
        || "aucun point".to_owned(),
        |l| format!("{scale} {l} dans les 4 compétences"),
    )
}

fn education_fr(e: EducationLevel) -> &'static str {
    use EducationLevel::*;
    match e {
        LessThanSecondary => "moins que le secondaire",
        Secondary => "diplôme d’études secondaires",
        OneYearPostSecondary => "programme postsecondaire d’un an",
        TwoYearPostSecondary => "programme postsecondaire de deux ans",
        BachelorOrThreeYear => "baccalauréat ou programme de trois ans ou plus",
        TwoOrMoreCredentials => "deux diplômes ou plus",
        MasterOrProfessional => "maîtrise ou diplôme professionnel",
        Doctorate => "doctorat",
    }
}

/// The estimate for one response (`a` is its conservative profile).
fn estimate_of(r: &Row, a: &Applicant) -> Estimate {
    let mut e = a.clone();
    let mut assumptions = vec![
        "Estimation préalable d’après les réponses au formulaire : aucun résultat de test réel, ce n’est pas un pointage officiel.".to_owned(),
    ];
    let answer = |q| r.get(q).unwrap_or("non répondu");

    let levels = AssumedLevels {
        french: r.get(Q::French).and_then(assumed_level),
        english: r.get(Q::English).and_then(assumed_level),
        spouse_english: r
            .get(Q::SpouseEnglish)
            .and_then(assumed_level)
            .filter(|_| e.spouse.is_some()),
    };
    assumptions.push(format!(
        "Français {} → {} ; anglais {} → {} (table prudente du 2026-10-22).",
        quote(answer(Q::French)),
        level_text(levels.french, "NCLC"),
        quote(answer(Q::English)),
        level_text(levels.english, "NCLC/CLB").replace("NCLC/CLB", "CLB"),
    ));

    match r.get(Q::Education).and_then(assumed_education) {
        Some(level) => {
            e.education = level;
            assumptions.push(format!(
                "Scolarité {} → {} (le niveau le plus bas mentionné est retenu).",
                quote(answer(Q::Education)),
                education_fr(level)
            ));
        }
        None => assumptions.push(format!(
            "Scolarité {} non reconnue → {} (aucun point).",
            quote(answer(Q::Education)),
            education_fr(e.education)
        )),
    }

    if let Some(s) = e.spouse.as_mut() {
        let edu = assumed_spouse_education(answer(Q::SpouseEducation));
        s.education = Some(edu);
        assumptions.push(format!(
            "Époux : scolarité {} → {} ; anglais {} → {}.",
            quote(answer(Q::SpouseEducation)),
            education_fr(edu),
            quote(answer(Q::SpouseEnglish)),
            level_text(levels.spouse_english, "CLB"),
        ));
    }

    if e.eligibility.credential == Some(Credential::ForeignWithoutEca) {
        // A missing evaluation is a step to take, not a show-stopper.
        e.eligibility.credential = None;
        assumptions.push("Évaluation des diplômes (EDE) à obtenir : requise pour le Programme des travailleurs qualifiés.".into());
    }
    if r.get(Q::Relative).is_some_and(is_yes) {
        e.sibling_in_canada = true;
        assumptions.push(
            "Proche au Canada « Oui » compté comme frère ou sœur (15 points) : à confirmer.".into(),
        );
    }
    if r.get(Q::JobOffer).is_some_and(|o| !is_no(o)) {
        assumptions.push(
            "Offre d’emploi : aucun point compté (code CNP et admissibilité inconnus).".into(),
        );
    }

    let (mut stoppers, mut checks) = (Vec::new(), Vec::new());
    if let Some(c) = r.get(Q::Criminal) {
        let n = norm(c);
        if n.starts_with("condamne") {
            stoppers.push(format!(
                "Antécédent judiciaire déclaré {} : interdiction de territoire pour criminalité possible (réhabilitation ou permis de séjour temporaire à étudier).",
                quote(c)
            ));
        } else if n.starts_with("suspecte") {
            checks.push(format!(
                "Justice : {} : à examiner (interdiction de territoire possible).",
                quote(c)
            ));
        }
    }
    if levels.french.is_none() && levels.english.is_none() {
        stoppers.push(
            "Aucun niveau de langue suffisant déclaré (débutant ou non répondu) : les programmes d’Entrée express exigent un test NCLC/CLB 5 à 7 au minimum."
                .into(),
        );
    }
    if let Some(h) = r.get(Q::Health)
        && !is_no(h)
    {
        checks.push(format!(
            "Santé : {} : examen médical requis ; fardeau excessif ou danger pour la santé publique à évaluer.",
            quote(h)
        ));
    }
    if let Some(p) = r.get(Q::Province) {
        let n = norm(p);
        if n.contains("quebec") || n.contains("montreal") {
            checks.push(format!(
                "Destination {} : le Québec sélectionne ses propres travailleurs (hors Entrée express).",
                quote(p)
            ));
        }
    }
    if r.get(Q::JobOffer).is_some_and(|o| !is_no(o)) {
        checks.push(format!(
            "Offre d’emploi déclarée (poste : {}) : peut ouvrir d’autres voies ; à vérifier.",
            quote(answer(Q::JobOfferPost))
        ));
    }
    Estimate {
        applicant: e,
        levels,
        assumptions,
        stoppers,
        checks,
        job_offer_declared: r.get(Q::JobOffer).is_some_and(|o| !is_no(o)),
    }
}

/// A row that could not become a client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skipped {
    pub row: usize,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Parsed {
    pub responses: Vec<Response>,
    pub skipped: Vec<Skipped>,
}

pub const NOT_THIS_FORM: &str = "Ce fichier ne correspond pas au formulaire « AUDIT de votre profil » (colonne du nom introuvable).";

/// Parses a form export. `ext` is the lower-case extension; `today` is the
/// date ages are computed on.
pub fn parse(bytes: &[u8], ext: &str, today: &IsoDate) -> Result<Parsed, String> {
    let t = match ext {
        "xlsx" => read_xlsx(bytes)?,
        "csv" => read_csv(bytes)?,
        _ => return Err("Format de fichier non pris en charge.".into()),
    };
    parse_table(&t, today)
}

/// Lower case, plain spaces, no accents on the few letters the questions use.
fn norm(s: &str) -> String {
    s.replace('œ', "oe")
        .chars()
        .map(|c| match c {
            '\u{a0}' | '\u{202f}' => ' ',
            'é' | 'è' | 'ê' | 'É' => 'e',
            'à' | 'â' => 'a',
            'ç' => 'c',
            'î' | 'ï' => 'i',
            'ô' => 'o',
            'û' | 'ù' => 'u',
            '’' => '\'',
            c => c.to_ascii_lowercase(),
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// The questions used, by a distinctive part of their (normalized) text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Q {
    Id,
    Start,
    Name,
    Age,
    Status,
    Email,
    Marital,
    SpouseStatus,
    SpouseEducation,
    SpouseEnglish,
    SpouseJob,
    SpouseExperience,
    SpouseTest,
    Criminal,
    Health,
    Job,
    Experience,
    CanadianDiploma,
    CanadianWork,
    Education,
    Eca,
    FrenchTest,
    JobOffer,
    JobOfferPost,
    Funds,
    French,
    English,
    Relative,
    Province,
}

const QUESTIONS: &[(Q, &str)] = &[
    (Q::Id, "id"),
    (Q::Start, "start time"),
    (Q::Name, "quel est votre nom et prenom"),
    (Q::Age, "quel est votre age et votre date de naissance"),
    (Q::Status, "quel est votre statut actuellement au canada"),
    (
        Q::Email,
        "adresse courriel utilisee lors de votre reglement",
    ),
    (Q::Marital, "quelle est votre situation matrimoniale"),
    (
        Q::SpouseStatus,
        "quel est le statut de votre conjoint actuellement au canada",
    ),
    (
        Q::SpouseEducation,
        "votre conjoint(e) a-t-il/elle un diplome d'etudes superieures",
    ),
    (
        Q::SpouseEnglish,
        "quel est le niveau d'anglais de votre conjoint",
    ),
    (Q::SpouseJob, "quel est le metier de votre conjoint"),
    (
        Q::SpouseExperience,
        "d'anciennete a votre conjoint dans son metier",
    ),
    (
        Q::SpouseTest,
        "votre conjoint(e) a-t-il/elle un test de langue",
    ),
    (
        Q::Criminal,
        "avez-vous deja ete condamne ou suspecte par la justice",
    ),
    (Q::Health, "avez-vous une maladie grave"),
    (Q::Job, "quel est votre metier et domaine d'activite"),
    (
        Q::Experience,
        "combien d'anciennete avez-vous dans votre metier",
    ),
    (Q::CanadianDiploma, "avez-vous un diplome canadien"),
    (
        Q::CanadianWork,
        "combien d'annees avez-vous deja travaille au canada",
    ),
    (Q::Education, "quel est votre diplome le plus eleve"),
    (Q::Eca, "avez-vous une evaluation des diplomes"),
    (Q::FrenchTest, "avez-vous fais un test de francais"),
    (
        Q::JobOffer,
        "avez-vous une offre d'emploi ou une promesse d'embauche",
    ),
    (Q::JobOfferPost, "pour quel poste cette offre d'emploi"),
    (Q::Funds, "taille du foyer et fonds d'etablissement"),
    (Q::French, "quel est votre niveau de francais"),
    (Q::English, "quel est votre niveau d'anglais"),
    (
        Q::Relative,
        "avez-vous un frere, une soeur, un parent ou un enfant majeur",
    ),
    (Q::Province, "quelle serait votre province de destination"),
];

/// Column of each question (answer columns only: "Points - " and
/// "Feedback - " columns of quiz-mode forms are ignored).
fn columns(header: &[String]) -> Vec<(Q, usize)> {
    let mut out = Vec::new();
    for (q, key) in QUESTIONS {
        let found = header.iter().enumerate().find(|(_, h)| {
            let h = norm(h);
            if h.starts_with("points - ") || h.starts_with("feedback - ") {
                return false;
            }
            // "id" must match exactly; questions by their distinctive part.
            if *q == Q::Id {
                h == *key
            } else {
                h.contains(key)
            }
        });
        if let Some((i, _)) = found {
            out.push((*q, i));
        }
    }
    out
}

struct Row<'a> {
    cells: &'a [String],
    cols: &'a [(Q, usize)],
}

impl Row<'_> {
    /// Trimmed answer, `None` if empty or the column is missing.
    fn get(&self, q: Q) -> Option<&str> {
        let (_, i) = self.cols.iter().find(|(c, _)| *c == q)?;
        self.cells
            .get(*i)
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
    }
}

/// "Non", "Aucun", "Jamais…", "NON", "Nob" and the like.
fn is_no(s: &str) -> bool {
    let n = norm(s);
    let first = n
        .split(|c: char| !c.is_alphanumeric())
        .next()
        .unwrap_or_default();
    [
        "non", "no", "nob", "aucun", "aucune", "jamais", "rien", "nothing",
    ]
    .contains(&first)
        || ["pas de", "pas ", "n'ai pas", "ne vis pas"]
            .iter()
            .any(|p| n.starts_with(p))
}

fn is_yes(s: &str) -> bool {
    matches!(norm(s).as_str(), "oui" | "yes")
}

/// Leading whole number in a short answer ("3 ans", "5 ans et plus" -> 5).
fn leading_number(s: &str) -> Option<u8> {
    let digits: String = s.trim().chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

const MONTHS: [&str; 12] = [
    "janvier",
    "fevrier",
    "mars",
    "avril",
    "mai",
    "juin",
    "juillet",
    "aout",
    "septembre",
    "octobre",
    "novembre",
    "decembre",
];

/// A full birth date in an answer: `d/m/yyyy` (also `-` or `.`) or
/// `d mois yyyy`. Two-digit years are not guessed.
fn birth_date(s: &str) -> Option<IsoDate> {
    let n = norm(s);
    let words: Vec<&str> = n
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '/' || c == '-' || c == '.'))
        .filter(|w| !w.is_empty())
        .collect();
    let mk = |d: &str, m: u32, y: &str| -> Option<IsoDate> {
        let (d, y): (u32, u32) = (d.parse().ok()?, y.parse().ok()?);
        (y >= 1900 && y.to_string().len() == 4).then_some(())?;
        IsoDate::parse(&format!("{y:04}-{m:02}-{d:02}"))
    };
    for w in &words {
        let parts: Vec<&str> = w.split(['/', '-', '.']).collect();
        if let [d, m, y] = parts.as_slice()
            && let Some(date) = m.parse().ok().and_then(|m| mk(d, m, y))
        {
            return Some(date);
        }
    }
    for i in 0..words.len().saturating_sub(2) {
        let d = words[i].trim_end_matches("er");
        if let Some(m) = MONTHS.iter().position(|m| *m == words[i + 1])
            && let Some(date) = mk(d, u32::try_from(m).ok()? + 1, words[i + 2])
        {
            return Some(date);
        }
    }
    None
}

/// Whole years from `birth` to `on`.
fn age_on(birth: &IsoDate, on: &IsoDate) -> Option<u8> {
    let (b, o) = (birth.as_str(), on.as_str());
    let y = |s: &str| s.get(0..4)?.parse::<i32>().ok();
    let mut age = y(o)? - y(b)?;
    if o.get(5..) < b.get(5..) {
        age -= 1;
    }
    u8::try_from(age).ok()
}

/// An age stated alone ("38 ans", "44"), when no birth date is given.
fn stated_age(s: &str) -> Option<u8> {
    let n = norm(s);
    let nums: Vec<u32> = n
        .split(|c: char| !c.is_ascii_digit())
        .filter(|w| !w.is_empty() && w.len() <= 2)
        .filter_map(|w| w.parse().ok())
        .filter(|a| (16..=99).contains(a))
        .collect();
    match nums.as_slice() {
        [a] => u8::try_from(*a).ok(),
        _ => None,
    }
}

/// Excel date serial ("46313.0695…") as YYYY-MM-DD.
fn excel_date(s: &str) -> Option<String> {
    let serial: f64 = s.trim().parse().ok()?;
    if !(1.0..2_958_465.0).contains(&serial) {
        return None;
    }
    // Serial 25569 is 1970-01-01 (Excel's 1900 date system).
    #[allow(clippy::cast_possible_truncation)]
    let days = serial.floor() as i32 - 25_569;
    IsoDate::from_days(days).map(|d| d.as_str().to_owned())
}

fn quote(s: &str) -> String {
    format!("« {} »", s.split_whitespace().collect::<Vec<_>>().join(" "))
}

/// "N personnes : J'ai au moins X $ CAD…" -> (N, X).
fn funds(s: &str) -> Option<(u8, u32)> {
    let n = norm(s);
    let size = leading_number(&n)?;
    let after = n.split("au moins").nth(1)?;
    let amount: String = after
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == ' ')
        .take_while(|c| *c != '$')
        .collect();
    let amount: u32 = amount.split_whitespace().collect::<String>().parse().ok()?;
    Some((size, amount))
}

/// A five-digit NOC code mentioned in a free-text answer.
fn noc_in(s: &str) -> Option<String> {
    s.split(|c: char| !c.is_ascii_digit())
        .find(|w| w.len() == 5)
        .map(str::to_owned)
}

/// The conservative starting profile: nothing assumed that would add points.
fn blank() -> Applicant {
    Applicant {
        marital_status: MaritalStatus::NeverMarried,
        spouse: None,
        age: 0,
        education: EducationLevel::LessThanSecondary,
        canadian_education: None,
        first_language: None,
        second_language: None,
        canadian_work_years: 0,
        canadian_work_by_noc: vec![],
        foreign_work_years: 0,
        certificate_of_qualification: false,
        job_offer: None,
        provincial_nomination: false,
        sibling_in_canada: false,
        eligibility: EligibilityFacts::default(),
    }
}

fn response(r: &Row, today: &IsoDate) -> Result<Response, String> {
    let full_name = r
        .get(Q::Name)
        .map(|n| n.split_whitespace().collect::<Vec<_>>().join(" "))
        .ok_or("nom absent")?;
    let mut a = blank();
    let mut notes = Vec::new();
    let mut note = |s: String| notes.push(s);

    let id = r.get(Q::Id).unwrap_or("?");
    let received = r.get(Q::Start).and_then(excel_date);
    note(format!(
        "Importé du formulaire (réponse n° {id}{}). Profil partiel : vérifier chaque valeur avant de vous fier au score.",
        received
            .as_deref()
            .map(|d| format!(", reçue le {d}"))
            .unwrap_or_default()
    ));

    // Age.
    match r.get(Q::Age) {
        Some(ans) => match birth_date(ans).and_then(|b| age_on(&b, today).map(|age| (b, age))) {
            Some((b, age)) => {
                a.age = age;
                note(format!(
                    "Âge : {age} ans au {}, d’après la date de naissance {} (réponse {}). Le profil ne change pas seul à l’anniversaire.",
                    today.as_str(),
                    b.as_str(),
                    quote(ans)
                ));
            }
            None => match stated_age(ans) {
                Some(age) => {
                    a.age = age;
                    note(format!(
                        "Âge : {age} ans selon la réponse {} (pas de date de naissance complète) : à confirmer.",
                        quote(ans)
                    ));
                }
                None => note(format!("Âge à saisir : réponse {} illisible.", quote(ans))),
            },
        },
        None => note("Âge à saisir : non répondu.".into()),
    }

    // Education: free text, needs the lawyer (and an ECA).
    note(format!(
        "Niveau de scolarité à choisir (laissé au plus bas) : réponse {}.",
        quote(r.get(Q::Education).unwrap_or("non répondu"))
    ));
    match (r.get(Q::CanadianDiploma), r.get(Q::Eca)) {
        (Some(d), _) if !is_no(d) => note(format!(
            "Diplôme canadien déclaré : {} : à préciser.",
            quote(d)
        )),
        (_, Some(e)) if norm(e).contains("pas encore") => {
            a.eligibility.credential = Some(Credential::ForeignWithoutEca);
            note("Évaluation des diplômes (EDE) pas encore faite.".into());
        }
        (_, Some(e)) => note(format!(
            "Évaluation des diplômes : {} : à préciser.",
            quote(e)
        )),
        _ => {}
    }

    // Languages: levels only, no test results.
    let french = r.get(Q::French).unwrap_or("non répondu");
    match r.get(Q::FrenchTest) {
        Some(t) if is_yes(t) => note(format!(
            "Test de français fait : saisir les résultats (niveau déclaré {}).",
            quote(french)
        )),
        Some(t) => note(format!(
            "Test de français : {} ; niveau déclaré {}. Aucun résultat de test saisi.",
            quote(t),
            quote(french)
        )),
        None => note(format!(
            "Niveau de français déclaré {} ; aucun résultat de test saisi.",
            quote(french)
        )),
    }
    if let Some(e) = r.get(Q::English) {
        note(format!(
            "Niveau d’anglais déclaré {} (aucun test saisi).",
            quote(e)
        ));
    }

    // Work.
    if let Some(cw) = r.get(Q::CanadianWork) {
        if norm(cw).starts_with('0') || is_no(cw) {
            a.canadian_work_years = 0;
        } else if let Some(n) = leading_number(cw) {
            a.canadian_work_years = n.min(5);
            note(format!(
                "Expérience au Canada : {} : confirmer qu’elle est qualifiée (FEER 0 à 3) et autorisée, et la ventiler par CNP.",
                quote(cw)
            ));
        } else {
            note(format!(
                "Expérience au Canada à saisir : réponse {}.",
                quote(cw)
            ));
        }
    }
    let job = r.get(Q::Job).unwrap_or("non répondu");
    match r.get(Q::Experience).and_then(leading_number) {
        Some(n) => {
            a.foreign_work_years = n.min(5).saturating_sub(a.canadian_work_years);
            note(format!(
                "Expérience dans le métier : {} sur 10 ans, comptée comme expérience à l’étranger : confirmer (qualifiée, temps plein) et saisir les emplois (CNP, dates, heures) pour l’admissibilité.",
                quote(r.get(Q::Experience).unwrap_or_default())
            ));
        }
        None => note("Expérience dans le métier à saisir.".into()),
    }
    note(match noc_in(job) {
        Some(code) => format!(
            "Métier déclaré {} : code CNP {code} mentionné, à vérifier.",
            quote(job)
        ),
        None => format!("Métier déclaré {} : code CNP à déterminer.", quote(job)),
    });

    // Job offer.
    if let Some(o) = r.get(Q::JobOffer)
        && !is_no(o)
    {
        note(format!(
            "Offre d’emploi déclarée {} (poste : {}) : vérifier qu’elle est admissible et saisir son code CNP.",
            quote(o),
            quote(r.get(Q::JobOfferPost).unwrap_or("non précisé"))
        ));
    }

    // Family.
    match r.get(Q::Marital).map(norm) {
        Some(m) if m.starts_with("celibataire") => a.marital_status = MaritalStatus::NeverMarried,
        Some(m) if m.contains("marie") || m.contains("union de fait") => {
            a.marital_status = MaritalStatus::Married;
            let status = r.get(Q::SpouseStatus);
            let spouse_canadian = status.is_some_and(|s| {
                let n = norm(s);
                n.contains("citoyen") || n.contains("resident permanent")
            });
            a.spouse = Some(Spouse {
                is_canadian_citizen_or_pr: spouse_canadian,
                accompanying: true,
                education: None,
                canadian_work_years: 0,
                language: None,
            });
            note("Marié ou en union de fait (le formulaire ne distingue pas) : préciser. Époux supposé accompagnant : à confirmer.".into());
            note(format!(
                "Époux : scolarité à choisir (réponse {}), anglais {}, test de langue {}, métier {} ({}).",
                quote(r.get(Q::SpouseEducation).unwrap_or("non répondu")),
                quote(r.get(Q::SpouseEnglish).unwrap_or("non répondu")),
                quote(r.get(Q::SpouseTest).unwrap_or("non répondu")),
                quote(r.get(Q::SpouseJob).unwrap_or("non répondu")),
                r.get(Q::SpouseExperience)
                    .unwrap_or("ancienneté non répondue"),
            ));
            if let Some(s) = status
                && !is_no(s)
            {
                note(format!(
                    "Statut de l’époux au Canada : {} : à vérifier (expérience ou études au Canada ?).",
                    quote(s)
                ));
            }
        }
        Some(_) => note(format!(
            "Situation matrimoniale à préciser : réponse {}.",
            quote(r.get(Q::Marital).unwrap_or_default())
        )),
        None => note("Situation matrimoniale à saisir.".into()),
    }
    match r.get(Q::Relative) {
        Some(x) if is_yes(x) => {
            a.eligibility.relative_in_canada = Some(true);
            note("Proche au Canada déclaré (frère, sœur, parent ou enfant majeur) : seuls un frère ou une sœur donnent des points au SCG ; préciser le lien et le statut.".into());
        }
        Some(_) => {}
        None => note("Proche au Canada : non répondu.".into()),
    }

    // Funds.
    match r.get(Q::Funds) {
        Some(f) if norm(f).contains("pas les fonds") => {
            a.eligibility.settlement_funds_cad = Some(0);
            note("Fonds d’établissement : le client déclare ne pas les avoir.".into());
        }
        Some(f) => match funds(f) {
            Some((size, amount)) => {
                a.eligibility.family_size = Some(size);
                a.eligibility.settlement_funds_cad = Some(amount);
                note(format!(
                    "Fonds : au moins {amount} $ pour un foyer de {size} personne(s) (seuil déclaré par le client) : vérifier le montant exact."
                ));
            }
            None => note(format!(
                "Fonds d’établissement à saisir : réponse {}.",
                quote(f)
            )),
        },
        None => note("Fonds d’établissement : non répondu.".into()),
    }

    // Destination and status.
    if let Some(p) = r.get(Q::Province) {
        let n = norm(p);
        if n.contains("quebec") || n.contains("montreal") {
            note(format!(
                "Destination {} : le Québec a son propre programme, hors Entrée express : à clarifier.",
                quote(p)
            ));
        } else {
            a.eligibility.intends_to_live_in_quebec = Some(false);
        }
    }
    if let Some(s) = r.get(Q::Status) {
        let n = norm(s);
        if is_no(s) || n.contains("touriste") || n.contains("pas encore") {
            a.eligibility.authorized_to_work_in_canada = Some(false);
        } else {
            note(format!(
                "Statut actuel au Canada : {} : à vérifier.",
                quote(s)
            ));
        }
    }

    // Admissibility answers only matter when something is declared.
    for (q, what) in [
        (Q::Criminal, "Antécédents judiciaires"),
        (Q::Health, "Santé"),
    ] {
        if let Some(x) = r.get(q)
            && !is_no(x)
        {
            note(format!(
                "{what} : réponse {} : à examiner (admissibilité).",
                quote(x)
            ));
        }
    }

    let email = r
        .get(Q::Email)
        .filter(|e| e.contains('@') && !e.contains(' '))
        .map(str::to_owned);
    let key: String = Sha256::digest(format!(
        "{id}|{}|{full_name}",
        r.get(Q::Start).unwrap_or_default()
    ))
    .iter()
    .map(|b| format!("{b:02x}"))
    .collect();
    let estimate = estimate_of(r, &a);
    Ok(Response {
        key,
        full_name,
        email,
        applicant: a,
        notes,
        estimate,
    })
}

/// Parses an already-read table (first row: headers).
pub fn parse_table(t: &[Vec<String>], today: &IsoDate) -> Result<Parsed, String> {
    let header = t.first().ok_or("Fichier vide.")?;
    let cols = columns(header);
    if !cols.iter().any(|(q, _)| *q == Q::Name) {
        return Err(NOT_THIS_FORM.into());
    }
    let mut parsed = Parsed {
        responses: vec![],
        skipped: vec![],
    };
    for (i, cells) in t.iter().enumerate().skip(1) {
        if cells.iter().all(|c| c.trim().is_empty()) {
            continue;
        }
        match response(&Row { cells, cols: &cols }, today) {
            Ok(r) => parsed.responses.push(r),
            Err(reason) => parsed.skipped.push(Skipped {
                row: i + 1,
                reason: reason.to_owned(),
            }),
        }
    }
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    fn d(s: &str) -> IsoDate {
        IsoDate::parse(s).unwrap()
    }

    #[test]
    fn birth_dates_and_ages() {
        let today = d("2026-10-21");
        for (ans, birth, age) in [
            ("26 - 21/08/2000", "2000-08-21", 26),
            ("29 ans le 9/08/1997", "1997-08-09", 29),
            ("31/01/1982 44", "1982-01-31", 44),
            ("36 ans, née le 10/04/1990", "1990-04-10", 36),
            ("45 ans (née le 11 août 1981)", "1981-08-11", 45),
            ("1er mars 1990", "1990-03-01", 36),
            ("30/10/1994", "1994-10-30", 31),
        ] {
            let b = birth_date(ans).unwrap_or_else(|| panic!("{ans}"));
            assert_eq!(b.as_str(), birth, "{ans}");
            assert_eq!(age_on(&b, &today), Some(age), "{ans}");
        }
        assert_eq!(birth_date("38 ans"), None);
        assert_eq!(
            birth_date("12/05/87"),
            None,
            "two-digit years are not guessed"
        );
        assert_eq!(stated_age("38 ans"), Some(38));
        assert_eq!(stated_age("44"), Some(44));
        assert_eq!(stated_age("entre 30 et 40"), None);
    }

    #[test]
    fn small_parsers() {
        assert!(
            is_no("Non ")
                && is_no("NON")
                && is_no("Nob")
                && is_no("Aucune")
                && is_no("Jamais condamné ou suspecté")
        );
        assert!(is_no("Rien pour le moment") && is_no("Aucun statut actuellement au Canada"));
        assert!(!is_no("Oui") && !is_no("Touriste") && !is_no("Nouveau-Brunswick"));
        assert_eq!(leading_number("5 ans et plus"), Some(5));
        assert_eq!(
            funds("2 personnes : J'ai au moins 20 000 $ CAD d'économies\u{a0}"),
            Some((2, 20_000))
        );
        assert_eq!(funds("Je n'ai pas les fonds disponibles."), None);
        assert_eq!(
            noc_in(" (Code 43203 )Agentes de services frontaliers"),
            Some("43203".into())
        );
        assert_eq!(
            excel_date("46313.06958333333").as_deref(),
            Some("2026-10-18")
        );
        assert_eq!(
            norm("Combien\u{a0}d'ancienneté  a"),
            "combien d'anciennete a"
        );
    }

    #[test]
    fn assumed_levels_and_education() {
        use EducationLevel::*;
        assert_eq!(assumed_level("Langue maternelle"), Some(9));
        assert_eq!(assumed_level("Bilingue"), Some(8));
        assert_eq!(assumed_level("Intermédiaire\u{a0}"), Some(6));
        assert_eq!(assumed_level("Débutant"), None);
        for (text, level) in [
            ("Master", Some(MasterOrProfessional)),
            ("Licence physique-chimie", Some(BachelorOrThreeYear)),
            (
                "Bachelor Marketing/communication",
                Some(BachelorOrThreeYear),
            ),
            (
                "Bac + 2 (DUT) - VAE niveau 7 (Master en cours) ",
                Some(TwoYearPostSecondary),
            ),
            (
                "Bac comptabilité et j’ai fait 2 ans BTS mais pas le diplôme ",
                Some(Secondary),
            ),
            (
                "Licence administration publique (bac+3)",
                Some(BachelorOrThreeYear),
            ),
            ("CAP vente", Some(Secondary)),
            ("Baccalauréat ", Some(Secondary)),
            ("Doctorat en chimie", Some(Doctorate)),
            ("Diplôme d’infirmière", None),
        ] {
            assert_eq!(assumed_education(text), level, "{text}");
        }
        assert_eq!(
            assumed_spouse_education("Il/elle n'a pas le baccalauréat"),
            LessThanSecondary
        );
        assert_eq!(
            assumed_spouse_education("Un brevet professionnel"),
            Secondary
        );
        assert_eq!(assumed_spouse_education("Un BTS"), TwoYearPostSecondary);
        assert_eq!(
            assumed_spouse_education("4 années d'université et +"),
            BachelorOrThreeYear
        );
        assert_eq!(assumed_spouse_education("Autre"), LessThanSecondary);
    }
}
