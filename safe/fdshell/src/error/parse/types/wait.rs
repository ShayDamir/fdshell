//! The `wait` family of `ParseError`, kept out of `types.rs` so the enums stay
//! under the line budget; the facade (`error/parse.rs`) re-exports this enum and
//! `ParseError` forwards it with the `Wait` variant.

/// [WaitError] Parser errors, the `wait` family (owned by `parse::wait_block`).
// Debug needed for impl Error (trait bound), not for display.
#[derive(displaydoc::Display, Debug)]
pub(crate) enum WaitError {
    /// `wait`: empty block (no arms)
    EmptyBlock,
    /// `wait`: empty arm pattern
    EmptyPattern,
    /// `wait`: missing ')' after arm pattern
    MissingCloseParen,
    /// `wait`: unknown arm pattern keyword
    UnknownPattern,
    /// `wait`: `readable`/`writable`/`finished` need an fd reference
    MissingFd,
    /// `wait`: fd reference must start with '%'
    FdRefPercent,
    /// `wait`: `after` needs a millisecond deadline
    MissingTimeout,
    /// `wait`: `after` deadline must be a non-negative integer
    InvalidTimeout,
    /// `wait`: unexpected token in arm pattern
    UnexpectedToken,
}

impl core::error::Error for WaitError {}
