#![allow(clippy::unwrap_used)] // test code may panic

use std::sync::Arc;

use audit::{DataCategory, MemorySink, Outcome};
use net::{Allowlist, Capped, Client, NetError, Request, redirect_allowed};
use url::Url;

fn allow() -> Allowlist {
    Allowlist::new(["www.canada.ca", "ircc.canada.ca", "api.anthropic.com"])
}

fn u(s: &str) -> Url {
    Url::parse(s).unwrap()
}

#[test]
fn allowlist_policy() {
    let a = allow();
    assert_eq!(
        a.check(&u("https://www.canada.ca/fr/page.html?q=1")),
        Ok("www.canada.ca".into())
    );
    assert_eq!(
        a.check(&u("https://WWW.Canada.CA/x")),
        Ok("www.canada.ca".into())
    );
    for (url, why) in [
        ("http://www.canada.ca/x", "https"),
        ("https://canada.ca/x", "not on the allowlist"),
        ("https://evil.www.canada.ca/x", "not on the allowlist"),
        ("https://www.canada.ca.evil.com/x", "not on the allowlist"),
        ("https://www.canada.ca:8443/x", "port"),
        ("https://user:pw@www.canada.ca/x", "credentials"),
        ("https://192.0.2.1/x", "IP address"),
        ("https://[2001:db8::1]/x", "IP address"),
        ("ftp://www.canada.ca/x", "https"),
    ] {
        let e = a.check(&u(url)).unwrap_err();
        assert!(e.contains(why), "{url}: {e}");
    }
}

#[test]
fn redirects() {
    let a = allow();
    assert!(redirect_allowed(&a, &u("https://ircc.canada.ca/x"), 0).is_ok());
    assert!(
        redirect_allowed(&a, &u("https://tracker.example.com/x"), 0).is_err(),
        "foreign host"
    );
    assert!(
        redirect_allowed(&a, &u("http://www.canada.ca/x"), 0).is_err(),
        "downgrade to http"
    );
    assert!(
        redirect_allowed(&a, &u("https://www.canada.ca/x"), 3).is_err(),
        "too many hops"
    );
}

#[test]
fn content_types() {
    use net::content_type_allowed as ok;
    assert!(ok(Some("text/html; charset=UTF-8"), &["text/html"]));
    assert!(ok(Some("TEXT/HTML"), &["text/html"]));
    assert!(!ok(Some("application/octet-stream"), &["text/html"]));
    assert!(!ok(None, &["application/json"]));
    assert!(ok(None, &[]), "no restriction");
}

#[test]
fn size_cap() {
    let mut c = Capped::new(10);
    c.push(b"12345").unwrap();
    c.push(b"67890").unwrap();
    assert_eq!(c.push(b"!"), Err(NetError::TooLarge(10)));
    assert_eq!(c.into_inner(), b"1234567890");
}

#[tokio::test]
async fn blocked_requests_are_audited_without_connecting() {
    let sink = Arc::new(MemorySink::default());
    let client = Client::new(allow(), sink.clone()).unwrap();
    let mut req = Request::get(
        u("https://example.com/secret?token=abc"),
        DataCategory::DeidentifiedClientData,
    );
    req.source_file = Some("reponses-client.xlsx".into());
    let e = client.send(req).await.unwrap_err();
    assert!(matches!(e, NetError::Blocked(_)));

    let records = sink.records();
    assert_eq!(records.len(), 1);
    let r = &records[0];
    assert_eq!(r.outcome, Outcome::Blocked);
    assert_eq!(r.host, "example.com", "host only: no path or query string");
    assert_eq!(r.category, DataCategory::DeidentifiedClientData);
    assert_eq!(r.source_file.as_deref(), Some("reponses-client.xlsx"));
    assert!(r.reason.as_deref().unwrap().contains("allowlist"));
}

/// Real HTTPS fetch of an allowlisted official page: run with --include-ignored.
#[tokio::test]
#[ignore = "network"]
async fn live_fetch_is_audited() {
    let sink = Arc::new(MemorySink::default());
    let client = Client::new(allow(), sink.clone()).unwrap();
    let r = client
        .send(Request::get(
            u("https://www.canada.ca/fr/immigration-refugies-citoyennete/services/immigrer-canada/entree-express/verifier-note/criteries-scg.html"),
            DataCategory::PublicRulesFetch,
        ))
        .await
        .unwrap();
    assert_eq!(r.status, 200);
    assert!(String::from_utf8_lossy(&r.body).contains("Points supplémentaires"));
    let rec = &sink.records()[0];
    assert_eq!(
        (rec.outcome, rec.status, rec.host.as_str()),
        (Outcome::Completed, Some(200), "www.canada.ca")
    );
    assert_eq!(rec.bytes_received, r.body.len() as u64);
}
