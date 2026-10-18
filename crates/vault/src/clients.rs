//! Client records. The profile is opaque JSON here (an `applicant::Applicant`
//! serialized by the app); identity fields are separate columns so the
//! profile can be sent de-identified.

use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

use crate::{Error, Vault};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Identity {
    pub full_name: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    /// The firm's file number, if any.
    pub file_reference: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientRecord {
    pub id: i64,
    pub created_at: String,
    pub updated_at: String,
    pub identity: Identity,
    /// JSON of the applicant profile.
    pub profile: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientSummary {
    pub id: i64,
    pub full_name: String,
    pub file_reference: Option<String>,
    pub updated_at: String,
}

/// Input limits (characters / bytes): generous for real data, small enough
/// that a hostile or corrupted import cannot bloat the vault.
pub const MAX_NAME: usize = 200;
pub const MAX_EMAIL: usize = 254;
pub const MAX_PHONE: usize = 40;
pub const MAX_FILE_REFERENCE: usize = 64;
pub const MAX_PROFILE_BYTES: usize = 256 * 1024;

fn too_long(v: &Option<String>, max: usize) -> bool {
    v.as_ref().is_some_and(|s| s.chars().count() > max)
}

impl Vault {
    /// Inserts (`id = None`) or updates a client. Returns its id.
    pub fn client_save(
        &self,
        id: Option<i64>,
        identity: &Identity,
        profile: &str,
        now: &str,
    ) -> Result<i64, Error> {
        if identity.full_name.trim().is_empty() {
            return Err(Error::Invalid("client name is required"));
        }
        if identity.full_name.chars().count() > MAX_NAME
            || too_long(&identity.email, MAX_EMAIL)
            || too_long(&identity.phone, MAX_PHONE)
            || too_long(&identity.file_reference, MAX_FILE_REFERENCE)
        {
            return Err(Error::Invalid("identity field too long"));
        }
        if profile.len() > MAX_PROFILE_BYTES {
            return Err(Error::Invalid("profile too large"));
        }
        let c = self.conn();
        match id {
            None => {
                c.execute(
                    "INSERT INTO clients (created_at, updated_at, full_name, email, phone, file_reference, profile)
                     VALUES (?1, ?1, ?2, ?3, ?4, ?5, ?6)",
                    params![now, identity.full_name.trim(), identity.email, identity.phone, identity.file_reference, profile],
                )?;
                Ok(c.last_insert_rowid())
            }
            Some(id) => {
                let n = c.execute(
                    "UPDATE clients SET updated_at = ?2, full_name = ?3, email = ?4, phone = ?5, file_reference = ?6, profile = ?7
                     WHERE id = ?1",
                    params![id, now, identity.full_name.trim(), identity.email, identity.phone, identity.file_reference, profile],
                )?;
                if n == 0 {
                    Err(Error::Invalid("no such client"))
                } else {
                    Ok(id)
                }
            }
        }
    }

    pub fn client_get(&self, id: i64) -> Result<Option<ClientRecord>, Error> {
        Ok(self
            .conn()
            .query_row(
                "SELECT id, created_at, updated_at, full_name, email, phone, file_reference, profile FROM clients WHERE id = ?1",
                [id],
                |r| {
                    Ok(ClientRecord {
                        id: r.get(0)?,
                        created_at: r.get(1)?,
                        updated_at: r.get(2)?,
                        identity: Identity { full_name: r.get(3)?, email: r.get(4)?, phone: r.get(5)?, file_reference: r.get(6)? },
                        profile: r.get(7)?,
                    })
                },
            )
            .optional()?)
    }

    /// Most recently updated first.
    pub fn client_list(&self) -> Result<Vec<ClientSummary>, Error> {
        let mut st = self
            .conn()
            .prepare("SELECT id, full_name, file_reference, updated_at FROM clients ORDER BY updated_at DESC, id DESC")?;
        let rows = st.query_map([], |r| {
            Ok(ClientSummary {
                id: r.get(0)?,
                full_name: r.get(1)?,
                file_reference: r.get(2)?,
                updated_at: r.get(3)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn client_delete(&self, id: i64) -> Result<bool, Error> {
        Ok(self
            .conn()
            .execute("DELETE FROM clients WHERE id = ?1", [id])?
            > 0)
    }
}
