//! The `${ … }` and `` ` … ` `` region scanners, in the same shape as
//! `paren_scan`'s parenthesized ones: quote- and escape-aware, so a nested
//! region is one unit for every scanner that consumes it (task #130).
//!
//! Each region type has a slice scanner (body text + consumed byte count) and
//! an `_at` form (the index to resume scanning at, `None` when the region does
//! not close) — the tokenizer uses the first, `scan::advance` the second.

mod backtick;
mod brace;

pub(crate) use backtick::scan_backtick;
pub(crate) use backtick::scan_backtick_at;
pub(crate) use brace::scan_brace;
pub(crate) use brace::scan_brace_at;

#[cfg(test)]
mod tests;
