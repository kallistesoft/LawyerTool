//! CI publisher for the web app: fetches the official pages through the
//! allowlisted client, rebuilds the candidate grid and writes the
//! [`rules_watcher::Published`] snapshot as JSON to the path given as the
//! only argument. Public pages only; no client data exists here.

use std::sync::Arc;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let Some(out) = std::env::args().nth(1) else {
        eprintln!("usage: watch-publish <out.json>");
        std::process::exit(2);
    };
    // The audit trail of these public fetches is the CI log itself.
    let sink = Arc::new(audit::MemorySink::default());
    let client = match net::Client::new(
        net::Allowlist::new(["www.canada.ca", "ircc.canada.ca"]),
        sink.clone(),
    ) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("client: {e}");
            std::process::exit(1);
        }
    };
    let fetched = rules_watcher::fetch_all(&client).await;
    let published = rules_watcher::publish(&fetched, audit::now_rfc3339());
    for r in sink.records() {
        println!("{} {} {:?} {:?}", r.method, r.host, r.outcome, r.status);
    }
    for s in &published.sources {
        if let Some(e) = &s.error {
            println!("échec {} : {e}", s.id);
        }
    }
    let json = match serde_json::to_vec_pretty(&published) {
        Ok(j) => j,
        Err(e) => {
            eprintln!("json: {e}");
            std::process::exit(1);
        }
    };
    if let Err(e) = std::fs::write(&out, json) {
        eprintln!("{out}: {e}");
        std::process::exit(1);
    }
    println!(
        "{} sources, {} en échec -> {out}",
        published.sources.len(),
        published
            .sources
            .iter()
            .filter(|s| s.error.is_some())
            .count()
    );
}
