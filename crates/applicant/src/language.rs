use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OfficialLanguage {
    English,
    French,
}

/// Approved tests offered by the calculator (q5i-a / q12i).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TestKind {
    CelpipGeneral,
    IeltsGeneral,
    PteCore,
    TefCanada,
    TcfCanada,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Skill {
    Speaking,
    Listening,
    Reading,
    Writing,
}

impl Skill {
    pub const ALL: [Skill; 4] = [
        Skill::Speaking,
        Skill::Listening,
        Skill::Reading,
        Skill::Writing,
    ];
}

impl TestKind {
    pub fn language(self) -> OfficialLanguage {
        match self {
            Self::CelpipGeneral | Self::IeltsGeneral | Self::PteCore => OfficialLanguage::English,
            Self::TefCanada | Self::TcfCanada => OfficialLanguage::French,
        }
    }

    /// Inclusive raw-score range and step for one skill, in this crate's units.
    ///
    /// IELTS bands are stored in tenths (6.5 → 65). CELPIP "M" is stored as 0.
    /// TEF Canada is checked against its widest scale here (0–699, used since
    /// 2019-10-01); the exact scale for the test date is enforced on conversion.
    pub fn score_range(self, skill: Skill) -> (u16, u16, u16) {
        use Skill::*;
        match (self, skill) {
            (Self::IeltsGeneral, _) => (0, 90, 5),
            (Self::CelpipGeneral, _) => (0, 12, 1),
            (Self::PteCore, _) => (10, 90, 1),
            (Self::TefCanada, _) => (0, 699, 1),
            (Self::TcfCanada, Speaking | Writing) => (0, 20, 1),
            (Self::TcfCanada, Listening | Reading) => (100, 699, 1),
        }
    }
}

/// Raw results of one language test.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LanguageTest {
    pub test: TestKind,
    /// Date the test was taken. Selects the TEF Canada scale (D-003) and
    /// drives the 2-year validity check.
    pub test_date: crate::IsoDate,
    pub speaking: u16,
    pub listening: u16,
    pub reading: u16,
    pub writing: u16,
}

impl LanguageTest {
    pub fn score(&self, skill: Skill) -> u16 {
        match skill {
            Skill::Speaking => self.speaking,
            Skill::Listening => self.listening,
            Skill::Reading => self.reading,
            Skill::Writing => self.writing,
        }
    }
}
