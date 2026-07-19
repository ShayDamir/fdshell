//! `ShortCStrError` — typed validation/bounds errors for `ShortCStr`.
//!
//! Replaces raw `i32` (EINVAL) returns so callers can match on the specific
//! problem instead of guessing from context.

use core::fmt;

/// Operation on a [`ShortCStr`](super::ShortCStr) failed.
///
/// Each variant describes a single, distinct cause.
#[derive(Debug)]
pub enum ShortCStrError {
    /// A NUL byte was provided where data bytes are expected
    /// (via `push()` or `from_vec()`).
    NulByte,
    /// Internal inconsistency: offset/length exceeds the backing buffer.
    /// Indicates a logic bug.
    BadState,
    /// Data exceeds the inline capacity (`INLINE_MAX`).
    /// For `from_vec()` this is normal — falls back to heap allocation.
    TooLarge,
    /// The bytes are not valid UTF-8 and cannot be parsed as a string.
    InvalidUtf8,
    /// The bytes are valid UTF-8 but could not be parsed as the
    /// requested type (e.g. `"abc"` for `i32`).
    FromStrFailed,
}

impl fmt::Display for ShortCStrError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NulByte => write!(f, "NUL byte in string data"),
            Self::BadState => write!(f, "internal state is inconsistent"),
            Self::TooLarge => write!(f, "data exceeds inline capacity"),
            Self::InvalidUtf8 => write!(f, "invalid UTF-8 in string data"),
            Self::FromStrFailed => write!(f, "string could not be parsed as the requested type"),
        }
    }
}

impl core::error::Error for ShortCStrError {}
