//! Audit trail of every outbound network attempt (brief, rule 8).
//!
//! A record says *that* something was sent, *where*, *what kind* of data and
//! *on whose behalf* (source file name) — never the content itself. Records
//! are stored in the encrypted vault.

use serde::{Deserialize, Serialize};

/// What kind of data a request carries. Content is never recorded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataCategory {
    /// Fetching public official pages (rules watcher). No client data.
    PublicRulesFetch,
    /// De-identified client answers sent to the Claude API.
    DeidentifiedClientData,
    /// Public official text sent to the Claude API (rules extraction).
    PublicTextForExtraction,
    /// Update check / download.
    AppUpdate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    /// Refused before any connection (policy).
    Blocked,
    /// Connection or protocol failure.
    Failed,
    /// Got an HTTP response (any status).
    Completed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditRecord {
    /// RFC 3339 UTC timestamp.
    pub at: String,
    pub method: String,
    /// Destination host only (no path, no query string).
    pub host: String,
    pub category: DataCategory,
    /// Name of the client file the data came from, if any (no path).
    pub source_file: Option<String>,
    pub outcome: Outcome,
    /// HTTP status, when there was a response.
    pub status: Option<u16>,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    /// Why it was blocked or failed (policy reason or error kind, no content).
    pub reason: Option<String>,
}

/// Where audit records go (the vault in the app, memory in tests).
pub trait AuditSink: Send + Sync {
    fn record(&self, r: &AuditRecord) -> Result<(), String>;
}

/// Keeps records in memory (tests, tooling).
#[derive(Default)]
pub struct MemorySink(pub std::sync::Mutex<Vec<AuditRecord>>);

impl AuditSink for MemorySink {
    fn record(&self, r: &AuditRecord) -> Result<(), String> {
        self.0
            .lock()
            .map_err(|_| "poisoned".to_owned())?
            .push(r.clone());
        Ok(())
    }
}

impl MemorySink {
    pub fn records(&self) -> Vec<AuditRecord> {
        self.0.lock().map(|v| v.clone()).unwrap_or_default()
    }
}

/// Current UTC time as RFC 3339 (`2026-10-18T14:03:07Z`).
pub fn now_rfc3339() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    rfc3339(secs)
}

/// Seconds since the Unix epoch → RFC 3339 UTC (civil-from-days, H. Hinnant).
pub fn rfc3339(secs: u64) -> String {
    let days = i64::try_from(secs / 86_400).unwrap_or(0);
    let rem = secs % 86_400;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn rfc3339() {
        assert_eq!(super::rfc3339(0), "1970-01-01T00:00:00Z");
        assert_eq!(super::rfc3339(1_792_345_145), "2026-10-18T17:39:05Z");
        assert_eq!(super::rfc3339(951_782_400), "2000-02-29T00:00:00Z");
    }
}
