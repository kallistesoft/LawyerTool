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
/// Spelled-out emails: "jean [at] example [dot] com", "jean (at) example.com".
static EMAIL_OBFUSCATED_RE: LazyLock<Regex> = LazyLock::new(|| {
    re(
        r"(?i)\b[a-z0-9._%+-]+\s*(?:\[at\]|\(at\)|\{at\}|\s+at\s+)\s*[a-z0-9-]+(?:\s*(?:\.|\[dot\]|\(dot\)|\{dot\}|\s+dot\s+)\s*[a-z0-9-]+)+\b",
    )
});
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

/// "T r e m b l a y", "T.r.e.m.b.l.a.y": each letter of a name part (4+
/// letters) followed by up to two separators.
fn spaced_pattern(literal: &str) -> String {
    let letters: Vec<char> = fold_accents(literal)
        .chars()
        .filter(|c| c.is_alphabetic())
        .collect();
    if letters.len() < 4 {
        return accent_pattern(literal);
    }
    letters
        .iter()
        .map(|c| accent_pattern(&c.to_string()))
        .collect::<Vec<_>>()
        .join(r"[\s._-]{1,2}")
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
        re: EMAIL_OBFUSCATED_RE.clone(),
        kind: Kind::Email,
        placeholder: EMAIL,
    });
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
                re: re(&format!(
                    r"(?i)\b(?:{}|{})\b",
                    accent_pattern(lit),
                    spaced_pattern(lit)
                )),
                kind: Kind::ClientName,
                placeholder: CLIENT,
            });
        }
    }
    out
}

/// Matching form of one character: invisible characters vanish, full-width
/// forms fold to ASCII, Cyrillic / Greek lookalikes map to Latin letters.
fn skeleton_char(c: char) -> Option<char> {
    match c {
        '\u{200b}'..='\u{200f}'
        | '\u{202a}'..='\u{202e}'
        | '\u{2060}'..='\u{2064}'
        | '\u{feff}'
        | '\u{ad}' => None,
        '\u{ff01}'..='\u{ff5e}' => char::from_u32(c as u32 - 0xfee0),
        'а' | 'α' => Some('a'),
        'е' | 'ε' => Some('e'),
        'о' | 'ο' => Some('o'),
        'р' | 'ρ' => Some('p'),
        'с' => Some('c'),
        'у' | 'γ' => Some('y'),
        'х' | 'χ' => Some('x'),
        'і' | 'ι' => Some('i'),
        'ј' => Some('j'),
        'ѕ' => Some('s'),
        'А' | 'Α' => Some('A'),
        'В' | 'Β' => Some('B'),
        'Е' | 'Ε' => Some('E'),
        'К' | 'Κ' => Some('K'),
        'М' | 'Μ' => Some('M'),
        'Н' | 'Η' => Some('H'),
        'О' | 'Ο' => Some('O'),
        'Р' | 'Ρ' => Some('P'),
        'С' => Some('C'),
        'Т' | 'Τ' => Some('T'),
        'Х' | 'Χ' => Some('X'),
        'І' | 'Ι' => Some('I'),
        'Ν' => Some('N'),
        'Ζ' => Some('Z'),
        'Υ' => Some('Y'),
        other => Some(other),
    }
}

/// The string the rules are matched against, with each of its characters'
/// byte range in the original.
fn skeleton_map(s: &str) -> (String, Vec<(usize, usize)>) {
    let mut sk = String::with_capacity(s.len());
    let mut map = Vec::with_capacity(s.len());
    for (i, c) in s.char_indices() {
        if let Some(k) = skeleton_char(c) {
            sk.push(k);
            map.push((i, i + c.len_utf8()));
        }
    }
    (sk, map)
}

/// Normalized matching form (exposed for tests and audits).
pub fn skeleton(s: &str) -> String {
    skeleton_map(s).0
}

/// Replaces every match of `re` in the skeleton of `text`, in the original.
fn replace_mapped(text: &str, re: &Regex, placeholder: &str) -> Option<String> {
    let (sk, map) = skeleton_map(text);
    let starts: Vec<usize> = sk.char_indices().map(|(i, _)| i).collect();
    let mut ranges = Vec::new();
    for m in re.find_iter(&sk) {
        if m.start() == m.end() {
            continue;
        }
        let first = starts.binary_search(&m.start()).ok()?;
        let last = starts.partition_point(|&b| b < m.end()).checked_sub(1)?;
        ranges.push((map[first].0, map[last].1));
    }
    if ranges.is_empty() {
        return None;
    }
    let mut out = text.to_owned();
    for (a, b) in ranges.into_iter().rev() {
        out.replace_range(a..b, placeholder);
    }
    Some(out)
}

fn scrub(s: &str, path: &str, rules: &[Rule], out: &mut Vec<Redaction>) -> String {
    let mut text = s.to_owned();
    for r in rules {
        if let Some(t) = replace_mapped(&text, &r.re, r.placeholder) {
            text = t;
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
