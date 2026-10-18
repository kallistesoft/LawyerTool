//! Deterministic CRS and Express Entry eligibility engine.
//!
//! Pure by design: no file, network or clock access. Rule values come from an
//! approved grid snapshot passed in by the caller.

pub mod crs;
pub mod eligibility;
pub mod grid;
pub mod language;
pub mod programs;
pub mod rulings;
