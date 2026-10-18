use serde::{Deserialize, Serialize};

/// Calendar date, `YYYY-MM-DD`. Lexicographic order is chronological order.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct IsoDate(String);

fn days_in_month(year: u16, month: u8) -> u8 {
    match month {
        2 if (year.is_multiple_of(4) && !year.is_multiple_of(100)) || year.is_multiple_of(400) => {
            29
        }
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

impl IsoDate {
    pub fn parse(s: &str) -> Option<Self> {
        let b = s.as_bytes();
        let shape = b.len() == 10
            && b[4] == b'-'
            && b[7] == b'-'
            && b.iter()
                .enumerate()
                .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit());
        if !shape {
            return None;
        }
        let (y, m, d) = (
            s[..4].parse().ok()?,
            s[5..7].parse().ok()?,
            s[8..].parse().ok()?,
        );
        Self::from_ymd(y, m, d)
    }

    pub fn from_ymd(year: u16, month: u8, day: u8) -> Option<Self> {
        let ok = (1900..=9999).contains(&year)
            && (1..=12).contains(&month)
            && (1..=days_in_month(year, month)).contains(&day);
        ok.then(|| Self(format!("{year:04}-{month:02}-{day:02}")))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn ymd(&self) -> (u16, u8, u8) {
        // Always valid: constructed only through `from_ymd`.
        let n = |r: std::ops::Range<usize>| self.0[r].parse().unwrap_or(0);
        (n(0..4), n(5..7) as u8, n(8..10) as u8)
    }

    /// Same calendar day `years` later; 29 February becomes 28 February.
    pub fn plus_years(&self, years: u16) -> Self {
        let (y, m, d) = self.ymd();
        let y = y.saturating_add(years).min(9999);
        let d = d.min(days_in_month(y, m));
        Self(format!("{y:04}-{m:02}-{d:02}"))
    }

    pub fn next_day(&self) -> Self {
        let (y, m, d) = self.ymd();
        if d < days_in_month(y, m) {
            Self(format!("{y:04}-{m:02}-{:02}", d + 1))
        } else if m < 12 {
            Self(format!("{y:04}-{:02}-01", m + 1))
        } else {
            Self(format!("{:04}-01-01", y + 1))
        }
    }

    pub fn prev_day(&self) -> Self {
        let (y, m, d) = self.ymd();
        if d > 1 {
            Self(format!("{y:04}-{m:02}-{:02}", d - 1))
        } else if m > 1 {
            Self(format!(
                "{y:04}-{:02}-{:02}",
                m - 1,
                days_in_month(y, m - 1)
            ))
        } else {
            Self(format!("{:04}-12-31", y - 1))
        }
    }

    /// Days since 1970-01-01 (proleptic Gregorian; H. Hinnant's algorithm).
    pub fn to_days(&self) -> i32 {
        let (y, m, d) = self.ymd();
        let y = i32::from(y) - i32::from(m <= 2);
        let era = y.div_euclid(400);
        let yoe = y - era * 400;
        let m = i32::from(m);
        let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + i32::from(d) - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146_097 + doe - 719_468
    }

    pub fn from_days(days: i32) -> Option<Self> {
        let z = days + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = doy - (153 * mp + 2) / 5 + 1;
        let m = if mp < 10 { mp + 3 } else { mp - 9 };
        let y = yoe + era * 400 + i32::from(m <= 2);
        Self::from_ymd(
            u16::try_from(y).ok()?,
            u8::try_from(m).ok()?,
            u8::try_from(d).ok()?,
        )
    }

    /// The same calendar day `years` earlier; 29 February becomes 28 February.
    pub fn minus_years(&self, years: u16) -> Self {
        let (y, m, d) = self.ymd();
        let y = y.saturating_sub(years).max(1900);
        let d = d.min(days_in_month(y, m));
        Self(format!("{y:04}-{m:02}-{d:02}"))
    }

    /// Whole years from `self` to `later` (age on a date, validity periods).
    pub fn years_until(&self, later: &IsoDate) -> u16 {
        let (y0, m0, d0) = self.ymd();
        let (y1, m1, d1) = later.ymd();
        let full = (m1, d1) >= (m0, d0);
        y1.saturating_sub(y0)
            .saturating_sub(u16::from(!full && y1 > y0))
    }
}

impl TryFrom<String> for IsoDate {
    type Error = String;
    fn try_from(s: String) -> Result<Self, String> {
        Self::parse(&s).ok_or_else(|| format!("invalid date {s:?}, expected YYYY-MM-DD"))
    }
}

impl From<IsoDate> for String {
    fn from(d: IsoDate) -> String {
        d.0
    }
}
