//! The `wait` family of `ParseError`, kept out of `types.rs` so the enums stay
//! under the line budget; the facade (`error/parse.rs`) re-exports this enum and
//! `ParseError` forwards it with the `Wait` variant.

/// [WaitError] Parser errors, the `wait` family (owned by `parse::wait_block`).
// Debug needed for impl Error (trait bound), not for display.
#[derive(displaydoc::Display, Debug)]
pub(crate) enum WaitError {
    /// `wait`: empty block (no arms)
    WaitEmptyBlock,
    /// `wait`: empty arm pattern
    WaitEmptyPattern,
    /// `wait`: missing ')' after arm pattern
    WaitMissingCloseParen,
    /// `wait`: unknown arm pattern keyword
    WaitUnknownPattern,
    /// `wait`: `readable`/`writable`/`finished` need an fd reference
    WaitMissingFd,
    /// `wait`: fd reference must start with '%'
    WaitFdRefPercent,
    /// `wait`: `after` needs a millisecond deadline
    WaitMissingTimeout,
    /// `wait`: `after` deadline must be a non-negative integer
    WaitInvalidTimeout,
    /// `wait`: unexpected token in arm pattern
    WaitUnexpectedToken,
}

impl core::error::Error for WaitError {}
