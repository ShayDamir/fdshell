//! Parser errors (parse/*.rs).
//!
//! `ParseError` covers parse-time errors without position info.
//! Position is attached separately via `ParsePosition` on the `Report`
//! (`error/parse/position.rs`); the enum itself lives in `types`.

pub(crate) mod position;
mod types;

pub(crate) use position::{
    ParsePosition, report_case_arm_close_paren, report_stray_closer, report_unbalanced_quote,
    report_unexpected_eof,
};
pub(crate) use types::ParseError;

impl core::error::Error for ParseError {}
