//! Shared ASCII byte constants for the byte-level scanners.

pub(crate) mod fold;

/// The `"` byte, shared by the quote-state scanners (see LESSONS.md on why
/// this is a constant rather than a `b'"'` literal at each use site).
pub(crate) const QUOTE: u8 = b'"';
