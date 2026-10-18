//! De-identification (brief, rule 3): before anything is sent to Claude,
//! strip names, contact details and ID numbers; show the exact payload; put
//! the identifiers back locally in what comes back.
//!
//! The applicant profile holds no identifiers by design (identity is stored
//! apart). This crate is the second line: it walks **every string** of the
//! JSON payload and replaces
//! - the client's own name (full and each part), email, phone and file
//!   reference, case- and accent-insensitively, on word boundaries;
//! - anything that looks like an email, a phone number or an ID number
//!   (SIN, UCI / long numbers, passport-like codes).
//!
//! Numbers stored as JSON numbers (scores, months) are not identifiers and are
//! left alone. Pure: no I/O.

use std::sync::LazyLock;

use regex::Regex;
use serde::Serialize;
use serde_json::Value;

/// The client's identifiers, from the vault (never part of the payload).
#[derive(Debug, Clone, Default)]
pub struct Identifiers {
    pub full_name: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub file_reference: Option<String>,
}

pub const CLIENT: &str = "[CLIENT]";
pub const CLIENT_EMAIL: &str = "[CLIENT_EMAIL]";
pub const CLIENT_PHONE: &str = "[CLIENT_PHONE]";
pub const FILE_REF: &str = "[FILE_REF]";
pub const EMAIL: &str = "[EMAIL]";
pub const PHONE: &str = "[PHONE]";
pub const ID_NUMBER: &str = "[ID_NUMBER]";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    ClientName,
    ClientEmail,
    ClientPhone,
    FileReference,
    Email,
    Phone,
    IdNumber,
}

/// One replacement: where (JSON pointer), what kind. The original value is
/// never recorded here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Redaction {
    pub path: String,
    pub kind: Kind,
    pub placeholder: &'static str,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Deidentified {
    pub payload: Value,
    pub redactions: Vec<Redaction>,
}

fn re(p: &str) -> Regex {
    // Patterns are constants or built from escaped input; a bad one is a bug.
    #[allow(clippy::expect_used)]
    Regex::new(p).expect("valid regex")
}

static EMAIL_RE: LazyLock<Regex> =
    LazyLock::new(|| re(r"(?i)\b[a-z0-9._%+-]+@[a-z0-9.-]+\.[a-z]{2,}\b"));
/// North American numbers: optional +1, area code, 7 digits; common separators.
static PHONE_RE: LazyLock<Regex> =
    LazyLock::new(|| re(r"(?:\+?1[\s.-]?)?\(?\b\d{3}\)?[\s.-]?\d{3}[\s.-]\d{4}\b"));
/// SIN-like (3-3-3 with separators), long digit runs (UCI is 8 or 10 digits),
/// passport-like (1–2 letters + 6–9 digits).
static ID_RES: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    vec![
        re(r"\b\d{3}[\s-]\d{3}[\s-]\d{3}\b"),
        re(r"\b\d{8,}\b"),
        re(r"\b[A-Za-z]{1,2}\d{6,9}\b"),
    ]
});

fn fold_accents(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'à' | 'â' | 'ä' | 'á' | 'ã' => 'a',
            'À' | 'Â' | 'Ä' | 'Á' | 'Ã' => 'A',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'É' | 'È' | 'Ê' | 'Ë' => 'E',
            'î' | 'ï' | 'í' | 'ì' => 'i',
            'Î' | 'Ï' | 'Í' | 'Ì' => 'I',
            'ô' | 'ö' | 'ó' | 'ò' | 'õ' => 'o',
            'Ô' | 'Ö' | 'Ó' | 'Ò' | 'Õ' => 'O',
            'û' | 'ü' | 'ú' | 'ù' => 'u',
            'Û' | 'Ü' | 'Ú' | 'Ù' => 'U',
            'ç' => 'c',
            'Ç' => 'C',
            'ñ' => 'n',
            'Ñ' => 'N',
            other => other,
        })
        .collect()
}

/// Accent-insensitive pattern for one literal: each foldable letter becomes a
/// class of its variants.
fn accent_pattern(literal: &str) -> String {
    let folded = fold_accents(literal);
    folded
        .chars()
        .map(|c| {
            let variants: &str = match c.to_ascii_lowercase() {
                'a' => "aàâäáã",
                'e' => "eéèêë",
                'i' => "iîïíì",
                'o' => "oôöóòõ",
                'u' => "uûüúù",
                'c' => "cç",
                'n' => "nñ",
                _ => "",
            };
            if variants.is_empty() {
                regex::escape(&c.to_string())
            } else {
                format!("[{variants}]")
            }
        })
        .collect()
}

struct Rule {
    re: Regex,
    kind: Kind,
    placeholder: &'static str,
}

fn digits(s: &str) -> String {
    s.chars().filter(char::is_ascii_digit).collect()
}

fn rules(ids: &Identifiers) -> Vec<Rule> {
    // Structured identifiers first (emails, phones, ID numbers, file number),
    // names last: a name part must not break an email address it appears in.
    let mut out = Vec::new();
    if let Some(e) = ids.email.as_deref().filter(|e| !e.trim().is_empty()) {
        out.push(Rule {
            re: re(&format!(r"(?i){}", regex::escape(e.trim()))),
            kind: Kind::ClientEmail,
            placeholder: CLIENT_EMAIL,
        });
    }
    out.push(Rule {
        re: EMAIL_RE.clone(),
        kind: Kind::Email,
        placeholder: EMAIL,
    });
    if let Some(p) = ids.phone.as_deref().map(digits).filter(|d| d.len() >= 7) {
        // The same digits with any separators in between.
        let pat: String = p
            .chars()
            .map(|d| d.to_string())
            .collect::<Vec<_>>()
            .join(r"[\s().+.-]*");
        out.push(Rule {
            re: re(&pat),
            kind: Kind::ClientPhone,
            placeholder: CLIENT_PHONE,
        });
    }
    out.push(Rule {
        re: PHONE_RE.clone(),
        kind: Kind::Phone,
        placeholder: PHONE,
    });
    if let Some(f) = ids
        .file_reference
        .as_deref()
        .filter(|f| !f.trim().is_empty())
    {
        out.push(Rule {
            re: re(&format!(r"(?i)\b{}\b", regex::escape(f.trim()))),
            kind: Kind::FileReference,
            placeholder: FILE_REF,
        });
    }
    for r in ID_RES.iter() {
        out.push(Rule {
            re: r.clone(),
            kind: Kind::IdNumber,
            placeholder: ID_NUMBER,
        });
    }
    let name = ids.full_name.trim();
    if !name.is_empty() {
        // Full name first, then each part of 2+ letters (initials are too
        // ambiguous to match safely).
        let mut parts: Vec<&str> = name
            .split(|c: char| c.is_whitespace() || c == '-')
            .filter(|p| p.chars().count() >= 2)
            .collect();
        parts.sort_by_key(|p| std::cmp::Reverse(p.chars().count()));
        for lit in std::iter::once(name).chain(parts) {
            out.push(Rule {
                re: re(&format!(r"(?i)\b{}\b", accent_pattern(lit))),
                kind: Kind::ClientName,
                placeholder: CLIENT,
            });
        }
    }
    out
}

fn scrub(s: &str, path: &str, rules: &[Rule], out: &mut Vec<Redaction>) -> String {
    let mut text = s.to_owned();
    for r in rules {
        if r.re.is_match(&text) {
            text = r.re.replace_all(&text, r.placeholder).into_owned();
            out.push(Redaction {
                path: path.to_owned(),
                kind: r.kind,
                placeholder: r.placeholder,
            });
        }
    }
    text
}

fn walk(v: &mut Value, path: &mut String, rules: &[Rule], out: &mut Vec<Redaction>) {
    match v {
        Value::String(s) => *s = scrub(s, path, rules, out),
        Value::Array(items) => {
            for (i, item) in items.iter_mut().enumerate() {
                let len = path.len();
                path.push_str(&format!("/{i}"));
                walk(item, path, rules, out);
                path.truncate(len);
            }
        }
        Value::Object(map) => {
            for (k, item) in map.iter_mut() {
                let len = path.len();
                path.push('/');
                path.push_str(&k.replace('~', "~0").replace('/', "~1"));
                walk(item, path, rules, out);
                path.truncate(len);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

/// Returns `payload` with every identifier replaced, and where it was found.
pub fn deidentify(mut payload: Value, ids: &Identifiers) -> Deidentified {
    let rules = rules(ids);
    let mut redactions = Vec::new();
    walk(&mut payload, &mut String::new(), &rules, &mut redactions);
    Deidentified {
        payload,
        redactions,
    }
}

/// Puts the client's identifiers back into text produced from a
/// de-identified payload (locally, after the response).
pub fn reattach(text: &str, ids: &Identifiers) -> String {
    let mut out = text.replace(CLIENT, ids.full_name.trim());
    if let Some(e) = &ids.email {
        out = out.replace(CLIENT_EMAIL, e);
    }
    if let Some(p) = &ids.phone {
        out = out.replace(CLIENT_PHONE, p);
    }
    if let Some(f) = &ids.file_reference {
        out = out.replace(FILE_REF, f);
    }
    out
}
