//! Form files already seen in the input folder, keyed by content
//! fingerprint, so the same export is never imported twice. Only metadata.

use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

use crate::{Error, Vault};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportOutcome {
    /// A client was created from the file.
    Imported,
    /// The file could not be imported (format, validation…); retried on
    /// the next "Analyser".
    Failed,
}

impl ImportOutcome {
    fn as_str(self) -> &'static str {
        match self {
            Self::Imported => "imported",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportRow {
    pub fingerprint: String,
    pub file_name: String,
    pub imported_at: String,
    pub client_id: Option<i64>,
    pub outcome: ImportOutcome,
    pub message: String,
}

impl Vault {
    /// Records (or replaces) the latest attempt for a fingerprint.
    pub fn import_record(&self, row: &ImportRow) -> Result<(), Error> {
        self.conn().execute(
            "INSERT INTO imports (fingerprint, file_name, imported_at, client_id, outcome, message)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(fingerprint) DO UPDATE SET file_name = ?2, imported_at = ?3, client_id = ?4, outcome = ?5, message = ?6",
            params![row.fingerprint, row.file_name, row.imported_at, row.client_id, row.outcome.as_str(), row.message],
        )?;
        Ok(())
    }

    /// The latest attempt for a fingerprint, if any.
    pub fn import_get(&self, fingerprint: &str) -> Result<Option<ImportRow>, Error> {
        Ok(self
            .conn()
            .query_row(
                "SELECT fingerprint, file_name, imported_at, client_id, outcome, message FROM imports WHERE fingerprint = ?1",
                [fingerprint],
                |r| {
                    let outcome: String = r.get(4)?;
                    Ok(ImportRow {
                        fingerprint: r.get(0)?,
                        file_name: r.get(1)?,
                        imported_at: r.get(2)?,
                        client_id: r.get(3)?,
                        outcome: if outcome == "imported" { ImportOutcome::Imported } else { ImportOutcome::Failed },
                        message: r.get(5)?,
                    })
                },
            )
            .optional()?)
    }

    /// Removes a setting from the key/value `meta` table.
    pub fn meta_delete(&self, key: &str) -> Result<(), Error> {
        self.conn()
            .execute("DELETE FROM meta WHERE key = ?1", [key])?;
        Ok(())
    }
}
