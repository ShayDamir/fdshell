//! `ShortCStrError` — typed validation/bounds errors for `ShortCStr`.
//!
//! Replaces raw `i32` (EINVAL) returns so callers can match on the specific
//! problem instead of guessing from context.

use core::fmt;

/// Operation on a [`ShortCStr`](super::ShortCStr) failed.
///
/// Each variant describes a single, distinct cause.
#[derive(Debug, PartialEq)]
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
}

impl fmt::Display for ShortCStrError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NulByte => write!(f, "NUL byte in string data"),
            Self::BadState => write!(f, "internal state is inconsistent"),
            Self::TooLarge => write!(f, "data exceeds inline capacity"),
        }
    }
}

impl core::error::Error for ShortCStrError {}

/// Bridge for `?` in `Result<_, i32>` functions.
impl From<ShortCStrError> for i32 {
    fn from(_: ShortCStrError) -> Self {
        libc::EINVAL
    }
}
