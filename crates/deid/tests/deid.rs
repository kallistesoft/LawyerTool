#![allow(clippy::unwrap_used)] // test code may panic

use deid::{Identifiers, Kind, deidentify, reattach};
use serde_json::json;

fn ids() -> Identifiers {
    Identifiers {
        full_name: "Hélène Tremblay-Côté".into(),
        email: Some("helene.tc@example.com".into()),
        phone: Some("(514) 555-0142".into()),
        file_reference: Some("2026-041".into()),
    }
}

fn s(v: &serde_json::Value) -> String {
    v.to_string()
}

#[test]
fn client_identifiers_everywhere_are_replaced() {
    let payload = json!({
        "note_hint": "Hélène TREMBLAY-COTE a travaillé chez Cote Inc.",
        "nested": [{ "free_text": "Joindre helene.tc@EXAMPLE.com ou 514.555.0142, dossier 2026-041" }],
        "other": "Mme Côté, née Tremblay"
    });
    let d = deidentify(payload, &ids());
    let out = s(&d.payload);
    for leaked in [
        "Hélène",
        "TREMBLAY",
        "Tremblay",
        "Côté",
        "COTE",
        "helene.tc",
        "555",
        "2026-041",
    ] {
        assert!(!out.contains(leaked), "{leaked} leaked: {out}");
    }
    assert!(
        out.contains("[CLIENT]")
            && out.contains("[CLIENT_EMAIL]")
            && out.contains("[CLIENT_PHONE]")
            && out.contains("[FILE_REF]")
    );
    assert!(
        d.redactions
            .iter()
            .any(|r| r.path == "/nested/0/free_text" && r.kind == Kind::ClientEmail)
    );
    // "Cote Inc." is a coincidence with a name part: replaced too (fail safe).
    assert!(out.contains("[CLIENT] Inc."));
}

#[test]
fn generic_identifier_patterns() {
    let d = deidentify(
        json!({
            "a": "NAS 046 454 286, IUC 1234567890, passeport AB1234567",
            "b": "autre courriel jean@example.org, tél. 1-438-555-0199"
        }),
        &Identifiers::default(),
    );
    let out = s(&d.payload);
    for leaked in [
        "046 454 286",
        "1234567890",
        "AB1234567",
        "jean@example.org",
        "555-0199",
    ] {
        assert!(!out.contains(leaked), "{leaked} leaked: {out}");
    }
    assert!(out.contains("[ID_NUMBER]") && out.contains("[EMAIL]") && out.contains("[PHONE]"));
}

#[test]
fn profile_data_is_left_alone() {
    // What an Applicant profile actually contains: enums, dates, NOC codes, numbers.
    let profile = json!({
        "marital_status": "married", "age": 34, "education": "master_or_professional",
        "first_language": { "test": "tef_canada", "test_date": "2025-06-01", "speaking": 470, "listening": 440, "reading": 440, "writing": 450 },
        "canadian_work_by_noc": [{ "noc": "21231", "months": 30 }],
        "eligibility": { "settlement_funds_cad": 25000, "work_history": [{ "noc": "21231", "start": "2023-01-01", "end": "2025-06-30" }] }
    });
    let d = deidentify(profile.clone(), &ids());
    assert_eq!(d.payload, profile);
    assert!(d.redactions.is_empty(), "{:?}", d.redactions);
}

#[test]
fn name_parts_need_word_boundaries() {
    let i = Identifiers {
        full_name: "Ali Noor".into(),
        ..Default::default()
    };
    let d = deidentify(
        json!({ "t": "Alimentation et Noordam ; Ali est présent" }),
        &i,
    );
    assert_eq!(
        d.payload["t"],
        "Alimentation et Noordam ; [CLIENT] est présent"
    );
}

#[test]
fn reattach_restores_identifiers_locally() {
    let note =
        "[CLIENT] (dossier [FILE_REF]) est admissible. Contact : [CLIENT_EMAIL], [CLIENT_PHONE].";
    assert_eq!(
        reattach(note, &ids()),
        "Hélène Tremblay-Côté (dossier 2026-041) est admissible. Contact : helene.tc@example.com, (514) 555-0142."
    );
}

#[test]
fn redaction_records_never_hold_the_original_value() {
    let d = deidentify(json!({ "x": "Hélène" }), &ids());
    let rec = serde_json::to_string(&d.redactions).unwrap();
    assert!(!rec.contains("Hélène"));
}
