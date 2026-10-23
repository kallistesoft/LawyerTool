#![allow(clippy::unwrap_used)] // test code may panic

//! Manual check on a real export, read in place (never copied): writes one
//! audit per response to `AUDIT_OUT`. Nothing is committed.
//! `FORM_SAMPLE=/path/export.xlsx AUDIT_OUT=/tmp/dir cargo test -p audit-doc --test real_export -- --ignored`

#[test]
#[ignore = "needs FORM_SAMPLE and AUDIT_OUT"]
fn real_export_audits() {
    let (Ok(path), Ok(out)) = (std::env::var("FORM_SAMPLE"), std::env::var("AUDIT_OUT")) else {
        return;
    };
    let today = applicant::IsoDate::parse("2026-10-22").unwrap();
    let parsed = form_import::parse(&std::fs::read(path).unwrap(), "xlsx", &today).unwrap();
    let ctx = audit_doc::Context {
        today,
        french_cutoff: Some(435),
        funds: None,
    };
    for r in parsed.responses {
        let form = audit_doc::FormRecord {
            answers: r.answers,
            age: r.age,
            lawyer: audit_doc::Lawyer::default(),
        };
        let doc = audit_doc::build(&r.full_name, &form, &ctx);
        let name = audit_doc::file_name(&r.full_name);
        std::fs::write(format!("{out}/{name}"), report::rich::to_docx(&doc)).unwrap();
    }
}
