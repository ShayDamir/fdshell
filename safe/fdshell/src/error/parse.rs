#![forbid(unsafe_code)]

//! Parser errors (parse/*.rs).

/// Position information for a parser error.
///
/// The `source_start` field holds the byte offset into the input where the
/// error occurred. For errors that cannot be localized to a specific position
/// (e.g. execution-phase errors that propagate through the call chain),
/// `source_start` is 0.
#[derive(Debug)]
pub(crate) struct ParseErrorInfo {
    /// Byte offset of the error in the input.
    pub(crate) source_start: usize,
}

impl core::fmt::Display for ParseErrorInfo {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "parse error at position {}", self.source_start)
    }
}

impl core::error::Error for ParseErrorInfo {}

/// Convert an `i32` error to `ParseErrorInfo`.
///
/// This is a lossy conversion — position information is lost and defaults to 0.
/// Used for execution-phase errors that propagate through the call chain
/// (run_one, loop_, if_exec) where the original parse position is not available.
pub(crate) fn to_parse_err(_: i32) -> ParseErrorInfo {
    ParseErrorInfo { source_start: 0 }
}

impl From<i32> for ParseErrorInfo {
    fn from(_: i32) -> Self {
        ParseErrorInfo { source_start: 0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_parse_err_defaults_to_zero() {
        let info = to_parse_err(42);
        assert_eq!(info.source_start, 0);
    }
}
