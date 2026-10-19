//! Form-results import: turns one exported form file into a client.
//!
//! The parser for the firm's real form is written once a sample export
//! (with fake data) is available; until then every file is refused with a
//! clear message and nothing is created.

use applicant::Applicant;
use vault::Identity;

/// Extensions accepted in the input folder.
pub const EXTENSIONS: [&str; 2] = ["xlsx", "csv"];

/// Why a form file could not be imported (user-facing, French, no content).
pub type ImportError = String;

pub const FORMAT_PENDING: &str =
    "Format du formulaire à configurer : en attente d’un exemple d’export (données fictives).";

/// Parses one form file. `ext` is the lower-case extension.
pub fn parse_form(_bytes: &[u8], _ext: &str) -> Result<(Identity, Applicant), ImportError> {
    Err(FORMAT_PENDING.to_owned())
}
