//! Input folder (form results): which files are new, imported or failed,
//! and the import itself.
//!
//! The page lists the folder the lawyer chose in Paramètres (one level,
//! plain files, read on demand, never modified) and hands each file's
//! metadata and bytes here. Files are tracked by SHA-256 so the same export
//! is never imported twice.

use applicant::Applicant;
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

pub const FORMAT_PENDING: &str =
    "Format du formulaire à configurer : en attente d’un exemple d’export (données fictives).";

/// Parses one form file. `ext` is the lower-case extension.
///
/// The parser for the firm's real form is written once a sample export
/// (with fake data) is available; until then every file is refused with a
/// clear message and nothing is created.
pub fn parse_form(_bytes: &[u8], _ext: &str) -> Result<(Identity, Applicant), String> {
    Err(FORMAT_PENDING.to_owned())
}

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

/// Imports one file (skipped if already imported). The file is only read.
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
    let (outcome, client_id, message) = match parse_form(bytes, &ext) {
        Ok((identity, applicant)) => {
            let profile =
                serde_json::to_string(&applicant).map_err(|_| CommandError::internal())?;
            let id = v.client_save(None, &identity, &profile, &now)?;
            (ImportOutcome::Imported, Some(id), "Client créé.".to_owned())
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

    #[test]
    fn failed_imports_are_listed_with_their_reason_and_retried() {
        let (mut v, _) = Vault::create(
            "a long test passphrase",
            vault_file::KdfParams::INSECURE_FAST_FOR_TESTS,
        )
        .unwrap();
        let r = import(&mut v, "a.csv", b"a,b\n1,2").unwrap();
        assert!(!r.ok);
        assert_eq!(r.message, FORMAT_PENDING);
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
        assert_eq!(files[0].message.as_deref(), Some(FORMAT_PENDING));
        assert!(import(&mut v, "../a.csv", b"x").is_err());
    }
}
