//! Settings folders: the input folder (form results to analyse) and the
//! output folder (reviewed reports).
//!
//! Folders are only ever chosen in the native dialog opened here; the
//! WebView never supplies a path. The input folder is read on demand only,
//! one level deep, and its files are never modified (brief rule revised
//! 2026-10-18: "read on demand, from the folder the user chose").

use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;
use vault::{ImportOutcome, ImportRow};

use crate::import;
use crate::session::{CommandError, Session};

pub const INPUT_KEY: &str = "input_dir";
pub const OUTPUT_KEY: &str = "output_dir";
/// Largest form file read (bytes).
pub const MAX_FILE: u64 = 10 * 1024 * 1024;
/// Most files listed from the input folder.
pub const MAX_ENTRIES: usize = 500;

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Which {
    Input,
    Output,
}

impl Which {
    fn key(self) -> &'static str {
        match self {
            Self::Input => INPUT_KEY,
            Self::Output => OUTPUT_KEY,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct FolderStatus {
    pub path: String,
    pub exists: bool,
    pub writable: bool,
}

#[derive(Debug, Serialize)]
pub struct Settings {
    pub input_dir: Option<FolderStatus>,
    pub output_dir: Option<FolderStatus>,
}

/// Whether a file can be created in `dir` (probe file, removed at once).
pub fn writable(dir: &Path) -> bool {
    let probe = dir.join(format!(".lawyertool-write-test-{}", std::process::id()));
    let ok = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)
        .is_ok();
    if ok {
        let _ = std::fs::remove_file(&probe);
    }
    ok
}

fn status(path: String, check_write: bool) -> FolderStatus {
    let p = Path::new(&path);
    let exists = p.is_dir();
    let writable = exists && check_write && writable(p);
    FolderStatus {
        path,
        exists,
        writable,
    }
}

fn settings_of(session: &Session) -> Settings {
    Settings {
        input_dir: session.setting(INPUT_KEY).map(|p| status(p, false)),
        output_dir: session.setting(OUTPUT_KEY).map(|p| status(p, true)),
    }
}

#[tauri::command]
pub fn settings_get(s: State<'_, Arc<Session>>) -> Result<Settings, CommandError> {
    s.with_vault(|_| Ok(()))?;
    Ok(settings_of(&s))
}

/// Opens the folder dialog and stores the choice. Returns the new settings
/// (unchanged if the dialog was cancelled).
#[tauri::command]
pub async fn settings_pick(app: AppHandle, which: Which) -> Result<Settings, CommandError> {
    let session = app.state::<Arc<Session>>().inner().clone();
    session.with_vault(|_| Ok(()))?;
    let title = match which {
        Which::Input => "Dossier des résultats de formulaires",
        Which::Output => "Dossier de sortie des évaluations",
    };
    let mut dialog = app.dialog().file().set_title(title);
    if let Some(current) = session
        .setting(which.key())
        .filter(|p| Path::new(p).is_dir())
    {
        dialog = dialog.set_directory(current);
    }
    let chosen = tauri::async_runtime::spawn_blocking(move || dialog.blocking_pick_folder())
        .await
        .map_err(|_| CommandError::internal())?;
    if let Some(chosen) = chosen {
        let dir = chosen
            .into_path()
            .map_err(|_| CommandError::new("invalid_path", "Dossier invalide."))?;
        let dir = std::fs::canonicalize(&dir)
            .map_err(|_| CommandError::new("invalid_path", "Dossier introuvable."))?;
        let path = dir
            .to_str()
            .ok_or_else(|| CommandError::new("invalid_path", "Nom de dossier non pris en charge."))?
            .to_owned();
        if matches!(which, Which::Output) && !writable(&dir) {
            return Err(CommandError::new(
                "not_writable",
                "Impossible d’écrire dans ce dossier.",
            ));
        }
        session.with_vault(|v| Ok(v.meta_set(which.key(), &path)?))?;
    }
    Ok(settings_of(&session))
}

#[tauri::command]
pub fn settings_clear(s: State<'_, Arc<Session>>, which: Which) -> Result<Settings, CommandError> {
    s.with_vault(|v| Ok(v.meta_delete(which.key())?))?;
    Ok(settings_of(&s))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FileStatus {
    New,
    Imported,
    Failed,
    TooLarge,
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

/// A candidate file, before its import status is known.
#[derive(Debug, PartialEq, Eq)]
pub struct Candidate {
    pub name: String,
    pub size: u64,
    pub modified: Option<String>,
    /// SHA-256 of the content (`None` if too large to read).
    pub fingerprint: Option<String>,
}

fn accepted_name(name: &str) -> bool {
    if name.starts_with('.') || name.starts_with("~$") {
        return false; // hidden files, Office lock files
    }
    Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| import::EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
}

fn sha256_file(path: &Path) -> std::io::Result<String> {
    let mut f = std::fs::File::open(path)?.take(MAX_FILE + 1);
    let mut buf = Vec::new();
    f.read_to_end(&mut buf)?;
    Ok(hex(&Sha256::digest(&buf)))
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn rfc3339(t: std::time::SystemTime) -> Option<String> {
    let secs = t.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs();
    Some(audit::rfc3339(secs))
}

/// Plain files directly inside `dir` with an accepted extension: no
/// subfolders, no symlinks, no hidden files; at most `MAX_ENTRIES`, sorted
/// by name.
pub fn list_dir(dir: &Path) -> std::io::Result<Vec<Candidate>> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if !accepted_name(&name) {
            continue;
        }
        // file_type() does not follow symlinks.
        let Ok(ft) = entry.file_type() else { continue };
        if !ft.is_file() {
            continue;
        }
        let Ok(meta) = entry.metadata() else { continue };
        let size = meta.len();
        let fingerprint = if size <= MAX_FILE {
            sha256_file(&entry.path()).ok()
        } else {
            None
        };
        out.push(Candidate {
            name,
            size,
            modified: meta.modified().ok().and_then(rfc3339),
            fingerprint,
        });
        if out.len() >= MAX_ENTRIES {
            break;
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

/// `name` as a plain file directly inside `dir` (no separators, `..`,
/// symlinks or other extensions).
pub fn resolve_in(dir: &Path, name: &str) -> Result<PathBuf, CommandError> {
    let bad = || CommandError::new("invalid_file", "Fichier refusé.");
    if Path::new(name).file_name().and_then(|n| n.to_str()) != Some(name) || !accepted_name(name) {
        return Err(bad());
    }
    let path = dir.join(name);
    let meta = std::fs::symlink_metadata(&path).map_err(|_| bad())?;
    if !meta.file_type().is_file() {
        return Err(bad());
    }
    let canon_dir = std::fs::canonicalize(dir).map_err(|_| bad())?;
    let canon = std::fs::canonicalize(&path).map_err(|_| bad())?;
    if canon.parent() != Some(canon_dir.as_path()) {
        return Err(bad());
    }
    Ok(path)
}

fn input_dir(session: &Session) -> Result<PathBuf, CommandError> {
    let dir = session.setting(INPUT_KEY).ok_or_else(|| {
        CommandError::new(
            "no_input_dir",
            "Choisissez d’abord le dossier des formulaires dans Paramètres.",
        )
    })?;
    let dir = PathBuf::from(dir);
    if !dir.is_dir() {
        return Err(CommandError::new(
            "input_dir_missing",
            "Le dossier des formulaires est introuvable.",
        ));
    }
    Ok(dir)
}

#[tauri::command]
pub async fn inbox_list(app: AppHandle) -> Result<Vec<InboxFile>, CommandError> {
    let session = app.state::<Arc<Session>>().inner().clone();
    session.with_vault(|_| Ok(()))?;
    let dir = input_dir(&session)?;
    let candidates = tauri::async_runtime::spawn_blocking(move || list_dir(&dir))
        .await
        .map_err(|_| CommandError::internal())?
        .map_err(|_| {
            CommandError::new(
                "read_failed",
                "Le dossier des formulaires n’a pas pu être lu.",
            )
        })?;
    session.with_vault(|v| {
        candidates
            .into_iter()
            .map(|c| {
                let row = match &c.fingerprint {
                    Some(f) => v.import_get(f)?,
                    None => None,
                };
                let (status, message, client_id) = match (&c.fingerprint, row) {
                    (None, _) => (FileStatus::TooLarge, None, None),
                    (Some(_), None) => (FileStatus::New, None, None),
                    (Some(_), Some(r)) if r.outcome == ImportOutcome::Imported => {
                        (FileStatus::Imported, None, r.client_id)
                    }
                    (Some(_), Some(r)) => (FileStatus::Failed, Some(r.message), None),
                };
                Ok(InboxFile {
                    name: c.name,
                    size: c.size,
                    modified: c.modified,
                    status,
                    message,
                    client_id,
                })
            })
            .collect()
    })
}

#[derive(Debug, Serialize)]
pub struct ImportResult {
    pub name: String,
    pub ok: bool,
    pub client_id: Option<i64>,
    pub message: String,
}

/// Imports the named files from the input folder (skipping those already
/// imported). Files are read, never modified.
#[tauri::command]
pub async fn inbox_import(
    app: AppHandle,
    names: Vec<String>,
) -> Result<Vec<ImportResult>, CommandError> {
    let session = app.state::<Arc<Session>>().inner().clone();
    session.with_vault(|_| Ok(()))?;
    if names.len() > MAX_ENTRIES {
        return Err(CommandError::new("invalid", "Trop de fichiers."));
    }
    let dir = input_dir(&session)?;
    let mut results = Vec::new();
    for name in names {
        let path = resolve_in(&dir, &name)?;
        let read = tauri::async_runtime::spawn_blocking(move || -> std::io::Result<Vec<u8>> {
            let mut buf = Vec::new();
            std::fs::File::open(&path)?
                .take(MAX_FILE + 1)
                .read_to_end(&mut buf)?;
            Ok(buf)
        })
        .await
        .map_err(|_| CommandError::internal())?;
        let Ok(bytes) = read else {
            results.push(ImportResult {
                name,
                ok: false,
                client_id: None,
                message: "Fichier illisible.".into(),
            });
            continue;
        };
        if bytes.len() as u64 > MAX_FILE {
            results.push(ImportResult {
                name,
                ok: false,
                client_id: None,
                message: "Fichier trop volumineux (10 Mo au plus).".into(),
            });
            continue;
        }
        let fingerprint = hex(&Sha256::digest(&bytes));
        let ext = Path::new(&name)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let result = session.with_vault(|v| {
            if let Some(r) = v
                .import_get(&fingerprint)?
                .filter(|r| r.outcome == ImportOutcome::Imported)
            {
                return Ok(ImportResult {
                    name: name.clone(),
                    ok: true,
                    client_id: r.client_id,
                    message: "Déjà importé.".into(),
                });
            }
            let now = audit::now_rfc3339();
            let (outcome, client_id, message) = match import::parse_form(&bytes, &ext) {
                Ok((identity, applicant)) => {
                    let profile =
                        serde_json::to_string(&applicant).map_err(|_| CommandError::internal())?;
                    let id = v.client_save(None, &identity, &profile, &now)?;
                    (ImportOutcome::Imported, Some(id), "Client créé.".to_owned())
                }
                Err(m) => (ImportOutcome::Failed, None, m),
            };
            v.import_record(&ImportRow {
                fingerprint: fingerprint.clone(),
                file_name: name.clone(),
                imported_at: now,
                client_id,
                outcome,
                message: message.clone(),
            })?;
            Ok(ImportResult {
                name: name.clone(),
                ok: outcome == ImportOutcome::Imported,
                client_id,
                message,
            })
        })?;
        results.push(result);
    }
    Ok(results)
}

/// `dir/name`, or `dir/stem-2.ext`, `-3`… if taken (never overwrites).
pub fn unique_path(dir: &Path, name: &str) -> PathBuf {
    let first = dir.join(name);
    if std::fs::symlink_metadata(&first).is_err() {
        return first;
    }
    let p = Path::new(name);
    let stem = p
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("evaluation");
    let ext = p.extension().and_then(|s| s.to_str());
    (2..)
        .map(|n| dir.join(ext.map_or_else(|| format!("{stem}-{n}"), |e| format!("{stem}-{n}.{e}"))))
        .find(|c| std::fs::symlink_metadata(c).is_err())
        .unwrap_or(first)
}

/// Writes `bytes` as a new file in `dir` (never overwrites). Returns its path.
pub fn write_new(dir: &Path, name: &str, bytes: &[u8]) -> std::io::Result<PathBuf> {
    use std::io::Write;
    for _ in 0..5 {
        let path = unique_path(dir, name);
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(mut f) => {
                f.write_all(bytes)?;
                return Ok(path);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    Err(std::io::Error::other("no free file name"))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    fn dir() -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("b-reponses.xlsx"), b"xlsx").unwrap();
        std::fs::write(d.path().join("a.CSV"), b"csv").unwrap();
        std::fs::write(d.path().join("notes.txt"), b"x").unwrap();
        std::fs::write(d.path().join(".cache.csv"), b"x").unwrap();
        std::fs::write(d.path().join("~$b-reponses.xlsx"), b"lock").unwrap();
        std::fs::create_dir(d.path().join("sub")).unwrap();
        std::fs::write(d.path().join("sub/inner.csv"), b"x").unwrap();
        std::fs::create_dir(d.path().join("folder.csv")).unwrap();
        d
    }

    #[test]
    fn lists_only_plain_form_files_one_level_deep() {
        let d = dir();
        #[cfg(unix)]
        {
            let outside = tempfile::tempdir().unwrap();
            std::fs::write(outside.path().join("secret.csv"), b"x").unwrap();
            std::os::unix::fs::symlink(
                outside.path().join("secret.csv"),
                d.path().join("link.csv"),
            )
            .unwrap();
            let names: Vec<String> = list_dir(d.path())
                .unwrap()
                .into_iter()
                .map(|c| c.name)
                .collect();
            assert_eq!(names, ["a.CSV", "b-reponses.xlsx"]);
        }
        let c = list_dir(d.path()).unwrap();
        assert_eq!(
            c[0].fingerprint.as_deref(),
            Some(hex(&Sha256::digest(b"csv")).as_str())
        );
        assert_eq!(c[0].size, 3);
    }

    #[test]
    fn large_files_are_listed_but_not_read() {
        let d = tempfile::tempdir().unwrap();
        let f = std::fs::File::create(d.path().join("big.csv")).unwrap();
        f.set_len(MAX_FILE + 1).unwrap();
        let c = list_dir(d.path()).unwrap();
        assert_eq!(c[0].fingerprint, None);
    }

    #[test]
    fn only_names_directly_inside_the_folder_resolve() {
        let d = dir();
        assert!(resolve_in(d.path(), "a.CSV").is_ok());
        for bad in [
            "../a.csv",
            "sub/inner.csv",
            "notes.txt",
            ".cache.csv",
            "folder.csv",
            "missing.csv",
            "",
            "..",
            "/etc/passwd",
        ] {
            assert!(resolve_in(d.path(), bad).is_err(), "{bad}");
        }
        #[cfg(unix)]
        {
            let outside = tempfile::tempdir().unwrap();
            std::fs::write(outside.path().join("secret.csv"), b"x").unwrap();
            std::os::unix::fs::symlink(
                outside.path().join("secret.csv"),
                d.path().join("link.csv"),
            )
            .unwrap();
            assert!(resolve_in(d.path(), "link.csv").is_err());
        }
    }

    #[test]
    fn output_files_are_never_overwritten() {
        let d = tempfile::tempdir().unwrap();
        let a = write_new(d.path(), "Evaluation-X.pdf", b"1").unwrap();
        let b = write_new(d.path(), "Evaluation-X.pdf", b"2").unwrap();
        let c = write_new(d.path(), "Evaluation-X.pdf", b"3").unwrap();
        assert_eq!(a.file_name().unwrap(), "Evaluation-X.pdf");
        assert_eq!(b.file_name().unwrap(), "Evaluation-X-2.pdf");
        assert_eq!(c.file_name().unwrap(), "Evaluation-X-3.pdf");
        assert_eq!(std::fs::read(&a).unwrap(), b"1");
        assert!(writable(d.path()));
        assert_eq!(
            std::fs::read_dir(d.path()).unwrap().count(),
            3,
            "probe file removed"
        );
    }

    #[test]
    fn the_import_parser_waits_for_a_sample() {
        assert_eq!(
            import::parse_form(b"a,b\n1,2", "csv").unwrap_err(),
            import::FORMAT_PENDING
        );
    }
}
