//! Value-by-value comparison of grid parts, and sentence diffs of prose.

use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::Value;

/// One value that differs between the approved grid and the candidate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Change {
    /// Readable path, rows identified by their key (e.g. `age[18]/without_spouse`).
    pub path: String,
    pub before: Option<String>,
    pub after: Option<String>,
    /// Official row text the new (or old) value comes from.
    pub source_text: Option<String>,
}

/// Fields that carry provenance, not rule values.
const SKIP: &[&str] = &[
    "cite",
    "cites",
    "sources",
    "id",
    "fetched",
    "sha256",
    "trades_cite",
    "extra_cite",
    "updated",
];
/// Fields that identify a row within a list.
const IDS: &[&str] = &[
    "key",
    "noc",
    "family_size",
    "clb",
    "test",
    "taken_from",
    "prefix",
];

fn row_id(v: &Value) -> Option<String> {
    let o = v.as_object()?;
    let parts: Vec<String> = IDS.iter().filter_map(|k| o.get(*k).map(compact)).collect();
    (!parts.is_empty()).then(|| parts.join(","))
}

fn compact(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => "∅".into(),
        other => other.to_string(),
    }
}

fn cite_text(v: &Value) -> Option<String> {
    v.get("cite")
        .and_then(|c| c.get("text"))
        .and_then(Value::as_str)
        .map(str::to_owned)
}

fn flatten(
    v: &Value,
    path: String,
    cite: Option<String>,
    out: &mut BTreeMap<String, (String, Option<String>)>,
) {
    match v {
        Value::Object(o) => {
            let cite = cite_text(v).or(cite);
            for (k, x) in o {
                if SKIP.contains(&k.as_str()) {
                    continue;
                }
                let p = if path.is_empty() {
                    k.clone()
                } else {
                    format!("{path}/{k}")
                };
                flatten(x, p, cite.clone(), out);
            }
        }
        Value::Array(items) => {
            for (i, x) in items.iter().enumerate() {
                let id = row_id(x).unwrap_or_else(|| i.to_string());
                flatten(x, format!("{path}[{id}]"), cite.clone(), out);
            }
        }
        leaf => {
            out.insert(path, (compact(leaf), cite));
        }
    }
}

/// Every rule value that was added, removed or changed.
pub fn grid_changes(approved: &Value, candidate: &Value) -> Vec<Change> {
    let (mut a, mut b) = (BTreeMap::new(), BTreeMap::new());
    flatten(approved, String::new(), None, &mut a);
    flatten(candidate, String::new(), None, &mut b);
    let mut out = Vec::new();
    for (path, (before, cite_a)) in &a {
        match b.get(path) {
            Some((after, _)) if after == before => {}
            Some((after, cite_b)) => out.push(Change {
                path: path.clone(),
                before: Some(before.clone()),
                after: Some(after.clone()),
                source_text: cite_b.clone().or(cite_a.clone()),
            }),
            None => out.push(Change {
                path: path.clone(),
                before: Some(before.clone()),
                after: None,
                source_text: cite_a.clone(),
            }),
        }
    }
    for (path, (after, cite_b)) in &b {
        if !a.contains_key(path) {
            out.push(Change {
                path: path.clone(),
                before: None,
                after: Some(after.clone()),
                source_text: cite_b.clone(),
            });
        }
    }
    out
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SentenceChange {
    Added(String),
    Removed(String),
}

fn sentences(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let b = text.as_bytes();
    for i in 0..b.len() {
        if matches!(b[i], b'.' | b';' | b':') && b.get(i + 1) == Some(&b' ') {
            out.push(text[start..=i].trim());
            start = i + 1;
        }
    }
    out.push(text[start..].trim());
    out.retain(|s| !s.is_empty());
    out
}

/// Sentences added or removed between two versions of a page (LCS).
pub fn sentence_diff(old: &str, new: &str) -> Vec<SentenceChange> {
    let (a, b) = (sentences(old), sentences(new));
    let (n, m) = (a.len(), b.len());
    let mut lcs = vec![vec![0u32; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lcs[i][j] = if a[i] == b[j] {
                lcs[i + 1][j + 1] + 1
            } else {
                lcs[i + 1][j].max(lcs[i][j + 1])
            };
        }
    }
    let (mut i, mut j, mut out) = (0, 0, Vec::new());
    while i < n && j < m {
        if a[i] == b[j] {
            i += 1;
            j += 1;
        } else if lcs[i + 1][j] >= lcs[i][j + 1] {
            out.push(SentenceChange::Removed(a[i].to_owned()));
            i += 1;
        } else {
            out.push(SentenceChange::Added(b[j].to_owned()));
            j += 1;
        }
    }
    out.extend(
        a[i..]
            .iter()
            .map(|s| SentenceChange::Removed((*s).to_owned())),
    );
    out.extend(
        b[j..]
            .iter()
            .map(|s| SentenceChange::Added((*s).to_owned())),
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sentence_diff_finds_added_and_removed() {
        let old = "Intro. Les points changent le 3 novembre. Fin.";
        let new = "Intro. Les points changent le 10 novembre. Nouvelle phrase. Fin.";
        assert_eq!(
            sentence_diff(old, new),
            vec![
                SentenceChange::Removed("Les points changent le 3 novembre.".into()),
                SentenceChange::Added("Les points changent le 10 novembre.".into()),
                SentenceChange::Added("Nouvelle phrase.".into()),
            ]
        );
        assert!(sentence_diff(old, old).is_empty());
    }

    #[test]
    fn rows_are_matched_by_key_not_position() {
        let a = serde_json::json!({ "age": [ { "key": 18, "points": 99, "cite": { "text": "18 ans | 90 | 99" } }, { "key": 19, "points": 105 } ] });
        // A row inserted before the others must not shift every comparison.
        let b = serde_json::json!({ "age": [ { "key": 17, "points": 0 }, { "key": 18, "points": 98, "cite": { "text": "18 ans | 90 | 98" } }, { "key": 19, "points": 105 } ] });
        let c = grid_changes(&a, &b);
        assert_eq!(c.len(), 3, "{c:#?}"); // 18 changed; 17 key + points added
        let changed = c
            .iter()
            .find(|x| x.path == "age[18]/points")
            .unwrap_or_else(|| unreachable!());
        assert_eq!(
            (changed.before.as_deref(), changed.after.as_deref()),
            (Some("99"), Some("98"))
        );
        assert_eq!(changed.source_text.as_deref(), Some("18 ans | 90 | 98"));
    }
}
