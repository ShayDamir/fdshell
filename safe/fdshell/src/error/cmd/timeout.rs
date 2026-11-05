//! The `timeout` builtin's own errors (task #526, split from `error/cmd.rs`);
//! the intercept dispatch boundary adds the statement layer's `CmdError::Parse`
//! frame, as the parse layer's reports do (LESSONS: "An unclosed block is a
//! statement-layer report, so it must name the construct").

/// [TimeoutError] `timeout` builtin errors
#[derive(displaydoc::Display, Debug)]
pub(crate) enum TimeoutError {
    /// timeout: '{value}' is not a seconds value
    TimeoutBadSeconds { value: sys::ShortCStr },
    /// timeout: missing seconds argument (usage: timeout <seconds> <cmd> [args ...])
    TimeoutMissingSeconds,
    /// timeout: missing command argument (usage: timeout <seconds> <cmd> [args ...])
    TimeoutMissingCommand,
    /// timeout: failed to launch the command
    TimeoutLaunch,
    /// timeout: failed to create the deadline timer
    TimeoutTimer,
    /// timeout: failed to poll the command or the deadline
    TimeoutPoll,
    /// timeout: failed to signal the command
    TimeoutSignal,
    /// timeout: failed to wait for the command
    TimeoutWait,
    /// {command}: `builtin`/`command` prefix is not supported
    BuiltinKeywordNotSupported { command: &'static str },
    /// {command}: captures are not supported
    CapturesNotSupported { command: &'static str },
    /// resolution error
    Resolve,
    /// impossible error state (should never occur)
    Never,
}

impl core::error::Error for TimeoutError {}
