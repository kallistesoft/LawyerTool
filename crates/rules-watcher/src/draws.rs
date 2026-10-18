//! Official Express Entry draws feed (ee_rounds_123_fr.json).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Draw {
    /// As published: usually digits, sometimes with a suffix ("91a", "91b":
    /// two draws on 2018-05-30).
    pub number: String,
    /// YYYY-MM-DD
    pub date: String,
    /// Draw type, e.g. "Catégorie de l'expérience canadienne".
    pub name: String,
    pub invitations: u32,
    /// Lowest CRS score invited.
    pub min_crs: u16,
}

#[derive(Deserialize)]
struct Feed {
    rounds: Vec<Round>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Round {
    draw_number: String,
    draw_date: String,
    draw_name: String,
    draw_size: String,
    #[serde(rename = "drawCRS")]
    draw_crs: String,
}

fn number<T: std::str::FromStr>(s: &str) -> Option<T> {
    let digits: String = s.chars().filter(char::is_ascii_digit).collect();
    (!digits.is_empty()).then(|| digits.parse().ok()).flatten()
}

/// Parses the feed, newest first. Malformed rows are an error (never guessed);
/// the checks match the plan: draw numbers unique, CRS within 0–1200.
pub fn parse_draws(json: &[u8]) -> Result<Vec<Draw>, String> {
    let feed: Feed = serde_json::from_slice(json).map_err(|e| format!("draws feed: {e}"))?;
    let mut out = Vec::with_capacity(feed.rounds.len());
    for r in feed.rounds {
        let bad = |what: &str| format!("draw {:?}: bad {what}", r.draw_number);
        let d = Draw {
            number: {
                let n = r.draw_number.trim();
                let ok = n.chars().next().is_some_and(|c| c.is_ascii_digit())
                    && n.chars().all(|c| c.is_ascii_alphanumeric());
                if ok {
                    n.to_owned()
                } else {
                    return Err(bad("number"));
                }
            },
            date: applicant::IsoDate::parse(&r.draw_date)
                .ok_or_else(|| bad("date"))?
                .as_str()
                .to_owned(),
            name: r.draw_name.trim().to_owned(),
            invitations: number(&r.draw_size).ok_or_else(|| bad("size"))?,
            min_crs: number::<u16>(&r.draw_crs)
                .filter(|c| *c <= 1200)
                .ok_or_else(|| bad("CRS"))?,
        };
        out.push(d);
    }
    // Newest first: by date, then by number (numeric part, then suffix).
    let key = |d: &Draw| {
        (
            d.date.clone(),
            number::<u32>(&d.number).unwrap_or(0),
            d.number.clone(),
        )
    };
    out.sort_by_key(|d| std::cmp::Reverse(key(d)));
    let mut seen = std::collections::HashSet::new();
    if !out.iter().all(|d| seen.insert(d.number.clone())) {
        return Err("draws feed: duplicate draw numbers".into());
    }
    Ok(out)
}
