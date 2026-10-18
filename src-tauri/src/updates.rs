//! Signed in-app updates (tauri-plugin-updater).
//!
//! - Enabled only when `plugins.updater` (public key + endpoint) is present
//!   in tauri.conf.json; until then `update_check` reports "not configured".
//! - The endpoint host must be on the app allowlist; every check and download
//!   is written to the audit log (category `app_update`), like all traffic.
//! - Channels: `stable` (configured endpoint) and `beta` (same URL with
//!   `/stable/` replaced by `/beta/`), chosen by the `update_channel` setting.
//! - `min_version` in latest.json: below it, the update is mandatory.
//! - The user always confirms; the app never restarts on its own.

use std::sync::Arc;

use audit::{AuditRecord, AuditSink, DataCategory, Outcome};
use serde::Serialize;
use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_updater::UpdaterExt;
use url::Url;

use crate::session::{ALLOWED_HOSTS, Session};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Channel {
    Stable,
    Beta,
}

impl Channel {
    pub fn parse(s: &str) -> Self {
        if s.eq_ignore_ascii_case("beta") {
            Self::Beta
        } else {
            Self::Stable
        }
    }
}

/// Endpoint for `channel`, derived from the configured stable endpoint.
pub fn channel_endpoint(stable: &Url, channel: Channel) -> Result<Url, String> {
    match channel {
        Channel::Stable => Ok(stable.clone()),
        Channel::Beta => {
            let s = stable.as_str();
            if !s.contains("/stable/") {
                return Err(
                    "stable endpoint must contain /stable/ to derive the beta channel".into(),
                );
            }
            Url::parse(&s.replacen("/stable/", "/beta/", 1)).map_err(|e| e.to_string())
        }
    }
}

/// `true` when `current` is below `min_version` (update mandatory).
pub fn is_mandatory(current: &str, raw_json: &serde_json::Value) -> bool {
    let Some(min) = raw_json.get("min_version").and_then(|v| v.as_str()) else {
        return false;
    };
    match (semver::Version::parse(current), semver::Version::parse(min)) {
        (Ok(c), Ok(m)) => c < m,
        // An unreadable min_version is treated as mandatory: fail safe.
        _ => true,
    }
}

#[derive(Debug, Serialize)]
pub struct UpdateInfo {
    pub configured: bool,
    pub available: bool,
    pub current: String,
    pub version: Option<String>,
    pub notes: Option<String>,
    pub mandatory: bool,
    pub channel: Channel,
}

fn configured_endpoint<R: Runtime>(app: &AppHandle<R>) -> Option<Url> {
    let cfg = app.config().plugins.0.get("updater")?;
    let first = cfg.get("endpoints")?.as_array()?.first()?.as_str()?;
    Url::parse(first).ok()
}

pub fn configured(context_plugins: &tauri::utils::config::PluginConfig) -> bool {
    context_plugins.0.contains_key("updater")
}

fn audit(
    session: &Arc<Session>,
    host: &str,
    outcome: Outcome,
    bytes: u64,
    reason: Option<String>,
) -> Result<(), String> {
    session.record(&AuditRecord {
        at: audit::now_rfc3339(),
        method: "GET".into(),
        host: host.to_owned(),
        category: DataCategory::AppUpdate,
        source_file: None,
        outcome,
        status: None,
        bytes_sent: 0,
        bytes_received: bytes,
        reason,
    })
}

struct Prepared {
    updater: tauri_plugin_updater::Updater,
    host: String,
    channel: Channel,
}

fn prepare<R: Runtime>(app: &AppHandle<R>) -> Result<Option<Prepared>, String> {
    let Some(stable) = configured_endpoint(app) else {
        return Ok(None);
    };
    let session = app.state::<Arc<Session>>();
    let channel = Channel::parse(&session.setting("update_channel").unwrap_or_default());
    let url = channel_endpoint(&stable, channel)?;
    let allow = net::Allowlist::new(ALLOWED_HOSTS.iter().copied());
    let host = match allow.check(&url) {
        Ok(h) => h,
        Err(reason) => {
            let host = url.host_str().unwrap_or("").to_owned();
            audit(&session, &host, Outcome::Blocked, 0, Some(reason.clone()))?;
            return Err(format!("update endpoint refused: {reason}"));
        }
    };
    let updater = app
        .updater_builder()
        .endpoints(vec![url])
        .map_err(|e| e.to_string())?
        .build()
        .map_err(|e| e.to_string())?;
    Ok(Some(Prepared {
        updater,
        host,
        channel,
    }))
}

#[tauri::command]
pub async fn update_check(app: AppHandle) -> Result<UpdateInfo, String> {
    let current = app.package_info().version.to_string();
    let Some(p) = prepare(&app)? else {
        return Ok(UpdateInfo {
            configured: false,
            available: false,
            current,
            version: None,
            notes: None,
            mandatory: false,
            channel: Channel::Stable,
        });
    };
    let session = app.state::<Arc<Session>>().inner().clone();
    let found = p.updater.check().await;
    match &found {
        Ok(_) => audit(&session, &p.host, Outcome::Completed, 0, None)?,
        Err(e) => audit(&session, &p.host, Outcome::Failed, 0, Some(e.to_string()))?,
    }
    let update = found.map_err(|e| e.to_string())?;
    Ok(match update {
        None => UpdateInfo {
            configured: true,
            available: false,
            current,
            version: None,
            notes: None,
            mandatory: false,
            channel: p.channel,
        },
        Some(u) => UpdateInfo {
            configured: true,
            available: true,
            mandatory: is_mandatory(&current, &u.raw_json),
            current,
            version: Some(u.version.clone()),
            notes: u.body.clone(),
            channel: p.channel,
        },
    })
}

/// Downloads, verifies (signature) and installs, then restarts. Only called
/// after the user clicked "Install and restart".
#[tauri::command]
pub async fn update_install(app: AppHandle) -> Result<(), String> {
    let p = prepare(&app)?.ok_or("updater is not configured")?;
    let session = app.state::<Arc<Session>>().inner().clone();
    let update = p
        .updater
        .check()
        .await
        .map_err(|e| e.to_string())?
        .ok_or("no update available")?;
    let mut received: u64 = 0;
    let result = update
        .download_and_install(|chunk, _total| received += chunk as u64, || {})
        .await;
    match &result {
        Ok(()) => audit(
            &session,
            &p.host,
            Outcome::Completed,
            received,
            Some(format!("installed {}", update.version)),
        )?,
        Err(e) => audit(
            &session,
            &p.host,
            Outcome::Failed,
            received,
            Some(e.to_string()),
        )?,
    }
    result.map_err(|e| e.to_string())?;
    // Close the vault (wipes the key) before restarting into the new version.
    session.lock();
    app.restart()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channels() {
        let stable = Url::parse("https://updates.example.ca/lawyertool/stable/latest.json")
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(
            channel_endpoint(&stable, Channel::Stable).ok(),
            Some(stable.clone())
        );
        assert_eq!(
            channel_endpoint(&stable, Channel::Beta)
                .ok()
                .map(|u| u.to_string()),
            Some("https://updates.example.ca/lawyertool/beta/latest.json".to_owned())
        );
        let odd =
            Url::parse("https://updates.example.ca/latest.json").unwrap_or_else(|_| unreachable!());
        assert!(channel_endpoint(&odd, Channel::Beta).is_err());
        assert_eq!(Channel::parse("BETA"), Channel::Beta);
        assert_eq!(Channel::parse(""), Channel::Stable);
    }

    #[test]
    fn min_version() {
        let j = |v: &str| serde_json::json!({ "version": "0.3.0", "min_version": v });
        assert!(is_mandatory("0.1.4", &j("0.2.0")));
        assert!(!is_mandatory("0.2.0", &j("0.2.0")));
        assert!(!is_mandatory(
            "0.1.4",
            &serde_json::json!({ "version": "0.3.0" })
        ));
        assert!(
            is_mandatory("0.1.4", &j("not-a-version")),
            "unreadable min_version fails safe"
        );
    }
}
