//! The app's only HTTP client (brief: all network access in the Rust
//! backend, to an allowlist only, every call audited).
//!
//! Policy, checked before any connection and on every redirect:
//! - `https` only, default port, no user info, no IP literals;
//! - exact host match against the allowlist (no wildcard, no subdomain);
//! - at most 3 redirects, each to an allowed URL.
//!
//! Responses are capped in size; bodies are never logged. Every attempt,
//! including blocked ones, produces an [`AuditRecord`] (metadata only).

use std::sync::Arc;
use std::time::Duration;

use audit::{AuditRecord, AuditSink, DataCategory, Outcome};
use url::{Host, Url};

pub const MAX_REDIRECTS: usize = 3;
pub const DEFAULT_MAX_BYTES: usize = 5 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NetError {
    #[error("blocked by network policy: {0}")]
    Blocked(String),
    #[error("request failed: {0}")]
    Failed(String),
    #[error("response larger than {0} bytes")]
    TooLarge(usize),
    #[error("could not record the audit entry: {0}")]
    Audit(String),
}

/// Exact host names the app may contact.
#[derive(Debug, Clone)]
pub struct Allowlist(Vec<String>);

impl Allowlist {
    pub fn new<I: IntoIterator<Item = S>, S: Into<String>>(hosts: I) -> Self {
        Self(
            hosts
                .into_iter()
                .map(|h| h.into().to_ascii_lowercase())
                .collect(),
        )
    }

    /// `Ok(host)` if `url` may be contacted, else the policy reason.
    pub fn check(&self, url: &Url) -> Result<String, String> {
        if url.scheme() != "https" {
            return Err(format!("scheme {:?} is not https", url.scheme()));
        }
        if !url.username().is_empty() || url.password().is_some() {
            return Err("credentials in URL".into());
        }
        if url.port().is_some() {
            return Err("non-default port".into());
        }
        let host = match url.host() {
            Some(Host::Domain(d)) => d.to_ascii_lowercase(),
            Some(Host::Ipv4(_) | Host::Ipv6(_)) => {
                return Err("IP address instead of host name".into());
            }
            None => return Err("no host".into()),
        };
        if self.0.contains(&host) {
            Ok(host)
        } else {
            Err(format!("host {host} is not on the allowlist"))
        }
    }
}

/// Redirect decision, separate from reqwest for testing.
pub fn redirect_allowed(allow: &Allowlist, next: &Url, hops_so_far: usize) -> Result<(), String> {
    if hops_so_far >= MAX_REDIRECTS {
        return Err(format!("more than {MAX_REDIRECTS} redirects"));
    }
    allow.check(next).map(|_| ())
}

/// Whether a `Content-Type` header matches one of `accept` (media type only,
/// case-insensitive; parameters like `charset` ignored). Empty `accept` = any.
pub fn content_type_allowed(content_type: Option<&str>, accept: &[&str]) -> bool {
    if accept.is_empty() {
        return true;
    }
    let Some(ct) = content_type else { return false };
    let media = ct
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    accept.iter().any(|a| a.eq_ignore_ascii_case(&media))
}

/// Collects a body in chunks, refusing anything over `max`.
#[derive(Debug)]
pub struct Capped {
    buf: Vec<u8>,
    max: usize,
}

impl Capped {
    pub fn new(max: usize) -> Self {
        Self {
            buf: Vec::new(),
            max,
        }
    }

    pub fn push(&mut self, chunk: &[u8]) -> Result<(), NetError> {
        if self.buf.len() + chunk.len() > self.max {
            return Err(NetError::TooLarge(self.max));
        }
        self.buf.extend_from_slice(chunk);
        Ok(())
    }

    pub fn into_inner(self) -> Vec<u8> {
        self.buf
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
}

impl Method {
    fn as_str(self) -> &'static str {
        match self {
            Method::Get => "GET",
            Method::Post => "POST",
        }
    }
}

pub struct Request {
    pub method: Method,
    pub url: Url,
    pub category: DataCategory,
    /// Client file name (no path) the data comes from, for the audit log.
    pub source_file: Option<String>,
    pub headers: Vec<(String, String)>,
    pub body: Option<Vec<u8>>,
    /// Accepted response media types (without parameters); empty = any.
    pub accept: Vec<&'static str>,
}

impl Request {
    pub fn get(url: Url, category: DataCategory) -> Self {
        Self {
            method: Method::Get,
            url,
            category,
            source_file: None,
            headers: vec![],
            body: None,
            accept: vec![],
        }
    }
}

#[derive(Debug)]
pub struct Response {
    pub status: u16,
    pub content_type: Option<String>,
    pub body: Vec<u8>,
}

pub struct Client {
    http: reqwest::Client,
    allow: Allowlist,
    audit: Arc<dyn AuditSink>,
    max_bytes: usize,
}

fn install_crypto_provider() {
    // Idempotent; fails only if another provider is already installed.
    let _ = rustls::crypto::ring::default_provider().install_default();
}

impl Client {
    pub fn new(allow: Allowlist, audit: Arc<dyn AuditSink>) -> Result<Self, NetError> {
        install_crypto_provider();
        let policy_allow = allow.clone();
        let http = reqwest::Client::builder()
            .https_only(true)
            .redirect(reqwest::redirect::Policy::custom(
                move |attempt| match redirect_allowed(
                    &policy_allow,
                    attempt.url(),
                    attempt.previous().len(),
                ) {
                    Ok(()) => attempt.follow(),
                    Err(reason) => attempt.error(reason),
                },
            ))
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(60))
            .no_proxy()
            .user_agent(concat!("LawyerTool/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|e| NetError::Failed(e.to_string()))?;
        Ok(Self {
            http,
            allow,
            audit,
            max_bytes: DEFAULT_MAX_BYTES,
        })
    }

    pub fn with_max_bytes(mut self, max: usize) -> Self {
        self.max_bytes = max;
        self
    }

    fn record(
        &self,
        req: &Request,
        host: String,
        outcome: Outcome,
        status: Option<u16>,
        received: usize,
        reason: Option<String>,
    ) -> Result<(), NetError> {
        let r = AuditRecord {
            at: audit::now_rfc3339(),
            method: req.method.as_str().into(),
            host,
            category: req.category,
            source_file: req.source_file.clone(),
            outcome,
            status,
            bytes_sent: req.body.as_ref().map_or(0, |b| b.len() as u64),
            bytes_received: received as u64,
            reason,
        };
        self.audit.record(&r).map_err(NetError::Audit)
    }

    /// Sends `req` if policy allows. The audit entry is written before the
    /// result is returned; if it cannot be written, the result is an error.
    pub async fn send(&self, req: Request) -> Result<Response, NetError> {
        let host = match self.allow.check(&req.url) {
            Ok(h) => h,
            Err(reason) => {
                let host = req.url.host_str().unwrap_or("").to_owned();
                self.record(&req, host, Outcome::Blocked, None, 0, Some(reason.clone()))?;
                return Err(NetError::Blocked(reason));
            }
        };

        let mut rb = match req.method {
            Method::Get => self.http.get(req.url.clone()),
            Method::Post => self.http.post(req.url.clone()),
        };
        for (k, v) in &req.headers {
            rb = rb.header(k, v);
        }
        if let Some(b) = &req.body {
            rb = rb.body(b.clone());
        }

        let result: Result<Response, NetError> = async {
            let mut resp = rb
                .send()
                .await
                .map_err(|e| NetError::Failed(error_kind(&e)))?;
            let status = resp.status().as_u16();
            let content_type = resp
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .map(str::to_owned);
            if !content_type_allowed(content_type.as_deref(), &req.accept) {
                return Err(NetError::Failed(format!(
                    "unexpected content type {}",
                    content_type.as_deref().unwrap_or("(none)")
                )));
            }
            let mut body = Capped::new(self.max_bytes);
            while let Some(chunk) = resp
                .chunk()
                .await
                .map_err(|e| NetError::Failed(error_kind(&e)))?
            {
                body.push(&chunk)?;
            }
            Ok(Response {
                status,
                content_type,
                body: body.into_inner(),
            })
        }
        .await;

        match &result {
            Ok(r) => self.record(
                &req,
                host,
                Outcome::Completed,
                Some(r.status),
                r.body.len(),
                None,
            )?,
            Err(e) => self.record(&req, host, Outcome::Failed, None, 0, Some(e.to_string()))?,
        }
        result
    }
}

/// Error kind without URLs, headers or bodies.
fn error_kind(e: &reqwest::Error) -> String {
    let kind = if e.is_redirect() {
        "redirect refused"
    } else if e.is_timeout() {
        "timeout"
    } else if e.is_connect() {
        "connection failed"
    } else if e.is_body() || e.is_decode() {
        "bad response body"
    } else if e.is_request() {
        "request error"
    } else {
        "network error"
    };
    kind.to_owned()
}
