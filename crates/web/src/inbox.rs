//! Input folder (form results): which files are new, imported or failed,
//! and the import itself.
//!
//! The page lists the folder the lawyer chose in Paramètres (one level,
//! plain files, read on demand, never modified) and hands each file's
//! metadata and bytes here. Files are tracked by SHA-256, and each form
//! response by its own key, so the same export (or the same response in a
//! later, longer export) is never imported twice.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use vault_file::{Identity, ImportOutcome, ImportRow, Vault};

use crate::{CommandError, Res, time};

/// Extensions accepted in the input folder.
pub const EXTENSIONS: [&str; 2] = ["xlsx", "csv"];
/// Largest form file read (bytes).
pub const MAX_FILE: u64 = 10 * 1024 * 1024;
/// Most files listed from the input folder.
pub const MAX_ENTRIES: usize = 500;

/// SHA-256 of a file's content (hex).
pub fn fingerprint(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Whether the page should list a file name: accepted extension, no hidden
/// files, no Office lock files, no path.
pub fn accepted_name(name: &str) -> bool {
    if name.is_empty()
        || name.starts_with('.')
        || name.starts_with("~$")
        || name.contains(['/', '\\'])
    {
        return false;
    }
    name.rsplit_once('.')
        .is_some_and(|(_, e)| EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FileStatus {
    New,
    Imported,
    Failed,
    TooLarge,
}

/// A file found by the page, before its import status is known.
#[derive(Debug, Deserialize)]
pub struct Candidate {
    pub name: String,
    pub size: u64,
    pub modified: Option<String>,
    /// SHA-256 of the content (`None` if too large to read).
    pub fingerprint: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct InboxFile {
    pub name: String,
    pub size: u64,
    pub modified: Option<String>,
    pub status: FileStatus,
    /// Message of the last failed attempt, if any.
    pub message: Option<String>,
    pub client_id: Option<i64>,
}

pub fn status(v: &Vault, files: Vec<Candidate>) -> Vec<InboxFile> {
    let mut out: Vec<InboxFile> = files
        .into_iter()
        .filter(|c| accepted_name(&c.name))
        .take(MAX_ENTRIES)
        .map(|c| {
            let row = c.fingerprint.as_deref().and_then(|f| v.import_get(f));
            let (status, message, client_id) = match (&c.fingerprint, row) {
                (None, _) => (FileStatus::TooLarge, None, None),
                (Some(_), None) => (FileStatus::New, None, None),
                (Some(_), Some(r)) if r.outcome == ImportOutcome::Imported => {
                    (FileStatus::Imported, None, r.client_id)
                }
                (Some(_), Some(r)) => (FileStatus::Failed, Some(r.message), None),
            };
            InboxFile {
                name: c.name,
                size: c.size,
                modified: c.modified,
                status,
                message,
                client_id,
            }
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

#[derive(Debug, Serialize)]
pub struct ImportResult {
    pub name: String,
    pub ok: bool,
    pub client_id: Option<i64>,
    pub message: String,
}

/// Key of one form response in the imports record (never a file fingerprint).
fn response_key(key: &str) -> String {
    format!("response:{key}")
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n > 1 { many } else { one })
}

/// Imports one file: one client per new form response, each with the
/// to-complete notes of `form_import` (skipped if already imported). The
/// file is only read.
pub fn import(v: &mut Vault, name: &str, bytes: &[u8]) -> Res<ImportResult> {
    if !accepted_name(name) {
        return Err(CommandError::new("invalid_file", "Fichier refusé."));
    }
    if bytes.len() as u64 > MAX_FILE {
        return Ok(ImportResult {
            name: name.into(),
            ok: false,
            client_id: None,
            message: "Fichier trop volumineux (10 Mo au plus).".into(),
        });
    }
    let fingerprint = fingerprint(bytes);
    if let Some(r) = v
        .import_get(&fingerprint)
        .filter(|r| r.outcome == ImportOutcome::Imported)
    {
        return Ok(ImportResult {
            name: name.into(),
            ok: true,
            client_id: r.client_id,
            message: "Déjà importé.".into(),
        });
    }
    let ext = name
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default();
    let now = time::now();
    let (outcome, client_id, message) = match form_import::parse(bytes, &ext, &time::today()) {
        Ok(parsed) => {
            let (mut created, mut known) = (Vec::new(), 0);
            for r in parsed.responses {
                let key = response_key(&r.key);
                if v.import_get(&key).is_some() {
                    known += 1;
                    continue;
                }
                let identity = Identity {
                    full_name: r.full_name,
                    email: r.email,
                    phone: None,
                    file_reference: None,
                };
                let profile =
                    serde_json::to_string(&r.applicant).map_err(|_| CommandError::internal())?;
                let id = v.client_save(None, &identity, &profile, &now)?;
                v.client_set_notes(id, r.notes)?;
                v.import_record(&ImportRow {
                    fingerprint: key,
                    file_name: name.into(),
                    imported_at: now.clone(),
                    client_id: Some(id),
                    outcome: ImportOutcome::Imported,
                    message: "Client créé.".into(),
                });
                created.push(id);
            }
            let mut parts = vec![plural(created.len(), "client créé", "clients créés")];
            if known > 0 {
                parts.push(plural(
                    known,
                    "réponse déjà importée",
                    "réponses déjà importées",
                ));
            }
            for s in &parsed.skipped {
                parts.push(format!("ligne {} ignorée ({})", s.row, s.reason));
            }
            let message = format!("{} : profils partiels, à compléter.", parts.join(", "));
            let single = (created.len() == 1).then(|| created[0]);
            (ImportOutcome::Imported, single, message)
        }
        Err(m) => (ImportOutcome::Failed, None, m),
    };
    v.import_record(&ImportRow {
        fingerprint,
        file_name: name.into(),
        imported_at: now,
        client_id,
        outcome,
        message: message.clone(),
    });
    Ok(ImportResult {
        name: name.into(),
        ok: outcome == ImportOutcome::Imported,
        client_id,
        message,
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn accepted_names() {
        for ok in ["a.CSV", "b-reponses.xlsx"] {
            assert!(accepted_name(ok), "{ok}");
        }
        for bad in [
            "notes.txt",
            ".cache.csv",
            "~$b.xlsx",
            "sub/inner.csv",
            "..",
            "",
            "csv",
        ] {
            assert!(!accepted_name(bad), "{bad}");
        }
    }

    const SAMPLE: &[u8] = include_bytes!("../../form-import/tests/fixtures/sample-form.xlsx");

    fn vault() -> Vault {
        Vault::create(
            "a long test passphrase",
            vault_file::KdfParams::INSECURE_FAST_FOR_TESTS,
        )
        .unwrap()
        .0
    }

    #[test]
    fn a_form_export_creates_one_client_per_response_once() {
        let mut v = vault();
        let r = import(&mut v, "AUDIT(1-3).xlsx", SAMPLE).unwrap();
        assert!(r.ok, "{}", r.message);
        assert_eq!(r.client_id, None, "several clients: none opened");
        assert!(
            r.message
                .starts_with("3 clients créés, ligne 5 ignorée (nom absent)"),
            "{}",
            r.message
        );
        let list = v.client_list();
        assert_eq!(list.len(), 3);
        let c = v.client_get(list[0].id).unwrap();
        assert!(!c.notes.is_empty() && c.notes[0].contains("Importé du formulaire"));
        assert!(c.identity.email.is_some() || c.identity.full_name == "Fictif Dominique");

        // The same file again: already imported.
        let again = import(&mut v, "AUDIT(1-3).xlsx", SAMPLE).unwrap();
        assert_eq!(again.message, "Déjà importé.");
        // A longer export with the same responses (different bytes): none duplicated.
        let mut longer = SAMPLE.to_vec();
        longer.extend_from_slice(b"\0");
        let r = import(&mut v, "AUDIT(1-4).xlsx", &longer).unwrap();
        assert!(
            r.message
                .starts_with("0 client créé, 3 réponses déjà importées"),
            "{}",
            r.message
        );
        assert_eq!(v.client_list().len(), 3);
    }

    #[test]
    fn failed_imports_are_listed_with_their_reason_and_retried() {
        let mut v = vault();
        let r = import(&mut v, "a.csv", b"a,b\n1,2").unwrap();
        assert!(!r.ok);
        assert_eq!(r.message, form_import::NOT_THIS_FORM);
        assert!(v.client_list().is_empty(), "nothing created");
        let files = status(
            &v,
            vec![
                Candidate {
                    name: "a.csv".into(),
                    size: 7,
                    modified: None,
                    fingerprint: Some(fingerprint(b"a,b\n1,2")),
                },
                Candidate {
                    name: "big.csv".into(),
                    size: MAX_FILE + 1,
                    modified: None,
                    fingerprint: None,
                },
                Candidate {
                    name: "new.xlsx".into(),
                    size: 1,
                    modified: None,
                    fingerprint: Some("00".into()),
                },
                Candidate {
                    name: "notes.txt".into(),
                    size: 1,
                    modified: None,
                    fingerprint: Some("11".into()),
                },
            ],
        );
        let st: Vec<_> = files.iter().map(|f| (f.name.as_str(), f.status)).collect();
        assert_eq!(
            st,
            [
                ("a.csv", FileStatus::Failed),
                ("big.csv", FileStatus::TooLarge),
                ("new.xlsx", FileStatus::New)
            ]
        );
        assert_eq!(
            files[0].message.as_deref(),
            Some(form_import::NOT_THIS_FORM)
        );
        assert!(import(&mut v, "../a.csv", b"x").is_err());
    }
}
