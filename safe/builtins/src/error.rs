//! Typed errors for builtin commands.
//!
//! Replaces raw `i32` errno returns so callers can match on the specific
//! problem instead of decoding integer constants.

/// Errors that can occur in builtin command dispatch and execution.
///
/// Each variant describes a single, distinct cause.
#[derive(displaydoc::Display, Debug)]
pub enum BuiltinError {
    /// user requested help text with `--help` / `-h`
    Help,
    /// missing argument {0}
    MissingArgument(&'static str),
    /// invalid argument {0}
    InvalidArgument(&'static str),
    /// syscall failed
    Syscall,
    /// unknown builtin
    Unknown,
    /// I/O error
    Io,
    /// failed to send fd to parent shell
    SendFdFailed,
    /// impossible error state (should never occur)
    Never,
}

impl core::error::Error for BuiltinError {}

/// Contextual suggestion attached to `InvalidArgument` errors.
///
/// Attached via `.attach_opaque()` so it appears in the error chain.
#[derive(Debug)]
pub struct Suggestion(pub &'static str);

/// Errors from parsing flag values (hex or named).
#[derive(displaydoc::Display, Debug)]
pub enum FlagParseError {
    /// invalid hexadecimal flag value
    HexParse,
    /// invalid UTF-8 in flag value
    Utf8,
    /// unknown flag name
    Unknown,
}

impl core::error::Error for FlagParseError {}

/// Errors from parsing mode values (octal/hex).
#[derive(displaydoc::Display, Debug)]
pub enum ModeParseError {
    /// failed to parse mode value
    ParseFailed,
    /// invalid UTF-8 in mode value
    Utf8,
}

impl core::error::Error for ModeParseError {}
