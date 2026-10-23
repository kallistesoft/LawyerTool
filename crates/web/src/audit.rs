//! « Générer les audits »: the lawyer's eligibility audit (Word) for a client
//! imported from the form, plus the inputs she sets first (CNP codes,
//! regulated profession, a free paragraph). See `crates/audit-doc`.

use audit_doc::{Context, FormRecord, Lawyer};
use serde::Serialize;
use vault_file::Vault;

use crate::{CommandError, Res, err, time};

fn record(v: &Vault, id: i64) -> Res<(vault_file::ClientRecord, FormRecord)> {
    let c = v
        .client_get(id)
        .ok_or_else(|| err("not_found", "Client introuvable."))?;
    let form = c
        .form
        .as_deref()
        .and_then(|j| serde_json::from_str::<FormRecord>(j).ok())
        .ok_or_else(|| {
            err(
                "no_form",
                "Ce client n’a pas été importé du formulaire : pas d’audit à générer.",
            )
        })?;
    Ok((c, form))
}

#[derive(Serialize)]
pub struct Inputs {
    lawyer: Lawyer,
    /// 5-digit code the client wrote, if any.
    suggested_noc: Option<String>,
    suggested_spouse_noc: Option<String>,
    married: bool,
    job: String,
    spouse_job: String,
}

/// The audit inputs of a client, or `None` if it was not imported from the form.
pub fn inputs(v: &Vault, id: i64) -> Res<Option<Inputs>> {
    let Ok((_, form)) = record(v, id) else {
        return Ok(None);
    };
    let facts = audit_doc::Facts::of(&FormRecord {
        lawyer: Lawyer::default(),
        ..form.clone()
    });
    Ok(Some(Inputs {
        suggested_noc: facts.noc.as_ref().map(|n| n.code.clone()),
        suggested_spouse_noc: facts.spouse_noc.as_ref().map(|n| n.code.clone()),
        married: facts.married,
        job: facts.a("job").to_owned(),
        spouse_job: facts.a("spouse_job").to_owned(),
        lawyer: form.lawyer,
    }))
}

fn valid_noc(code: &Option<String>) -> Res<Option<String>> {
    match code.as_deref().map(str::trim).filter(|c| !c.is_empty()) {
        None => Ok(None),
        Some(c) if c.len() == 5 && c.bytes().all(|b| b.is_ascii_digit()) => Ok(Some(c.to_owned())),
        Some(_) => Err(err(
            "invalid_noc",
            "Code CNP invalide : 5 chiffres attendus.",
        )),
    }
}

pub fn set_inputs(v: &mut Vault, id: i64, lawyer: Lawyer) -> Res<()> {
    let (_, mut form) = record(v, id)?;
    form.lawyer = Lawyer {
        noc: valid_noc(&lawyer.noc)?,
        spouse_noc: valid_noc(&lawyer.spouse_noc)?,
        regulated: lawyer.regulated.filter(|t| !t.trim().is_empty()),
        note: lawyer.note.filter(|t| !t.trim().is_empty()),
        ..lawyer
    };
    let json = serde_json::to_string(&form).map_err(|_| CommandError::internal())?;
    v.client_set_form(id, Some(json))?;
    Ok(())
}

/// A rendered audit for the page to save in the output folder.
#[derive(Serialize)]
pub struct AuditFile {
    pub name: String,
    #[serde(skip)]
    pub bytes: Vec<u8>,
}

/// Draws read to find the latest French-language draw.
const DRAWS_READ: usize = 60;

pub fn render(v: &Vault, id: i64) -> Res<AuditFile> {
    let (client, form) = record(v, id)?;
    let today = time::today();
    let grids = crate::watch::all_grids(v)?;
    let funds = rules_engine::grid::in_force(&grids, &today)
        .and_then(|g| g.programs.as_ref())
        .map(|p| p.funds.clone());
    let french_cutoff = v
        .draws_latest(DRAWS_READ)
        .into_iter()
        .find(|d| d.name.to_lowercase().contains("français"))
        .map(|d| d.min_crs);
    let ctx = Context {
        today,
        french_cutoff,
        funds,
    };
    let doc = audit_doc::build(&client.identity.full_name, &form, &ctx);
    Ok(AuditFile {
        name: audit_doc::file_name(&client.identity.full_name),
        bytes: report::rich::to_docx(&doc),
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    const SAMPLE: &[u8] = include_bytes!("../../form-import/tests/fixtures/sample-form.xlsx");

    #[test]
    fn imported_clients_get_an_audit_with_the_lawyers_inputs() {
        let (mut v, _) = Vault::create(
            "a long test passphrase",
            vault_file::KdfParams::INSECURE_FAST_FOR_TESTS,
        )
        .unwrap();
        let r = crate::inbox::import(&mut v, "AUDIT(1-3).xlsx", SAMPLE).unwrap();
        assert_eq!(r.client_ids.len(), 3);
        // The same file again: no new client, the same three ids.
        let again = crate::inbox::import(&mut v, "AUDIT(1-3).xlsx", SAMPLE).unwrap();
        assert_eq!(again.message, "Déjà importé.");
        assert_eq!(again.client_ids, r.client_ids);

        let alix = *r
            .client_ids
            .iter()
            .find(|id| v.client_get(**id).unwrap().identity.full_name == "TESTEUR Alix")
            .unwrap();
        let i = inputs(&v, alix).unwrap().unwrap();
        assert_eq!(
            i.suggested_noc.as_deref(),
            Some("31301"),
            "code written by the client"
        );
        assert!(!i.married);

        assert!(
            set_inputs(
                &mut v,
                alix,
                Lawyer {
                    noc: Some("12".into()),
                    ..Lawyer::default()
                }
            )
            .is_err()
        );
        set_inputs(
            &mut v,
            alix,
            Lawyer {
                noc: Some("31301".into()),
                noc_validated: true,
                regulated: Some("est réglementée au Canada".into()),
                ..Lawyer::default()
            },
        )
        .unwrap();
        let f = render(&v, alix).unwrap();
        assert_eq!(f.name, "Audit_TESTEUR.docx");
        let xml = String::from_utf8_lossy(&f.bytes);
        for want in [
            "TESTEUR Alix",
            "FEUILLE DE ROUTE",
            "CNP 31301",
            "est réglementée au Canada",
        ] {
            assert!(xml.contains(want), "{want}");
        }

        // A client typed by hand has no audit.
        let id = v
            .client_save(
                None,
                &vault_file::Identity {
                    full_name: "Manuel".into(),
                    email: None,
                    phone: None,
                    file_reference: None,
                },
                "{}",
                "t",
            )
            .unwrap();
        assert!(inputs(&v, id).unwrap().is_none());
        assert_eq!(render(&v, id).err().unwrap().code, "no_form");
    }
}
