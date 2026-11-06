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
    // Truly never: the four sites sit in proven classes — `timeout_cmd.rs`'s `_` arm
    // is a dispatch-completeness fallback (`validate_intercept` produces exactly two
    // variants, both mapped); its `Never -> Never` arm is the §4.17 re-context of
    // `CmdError::Never`, reachable only if that variant is; `borrow_mut()` clashes
    // need a borrow held across a nested borrow of the same cell, which the tree's
    // rule forbids; and `parse.rs`'s `as_bytes()` on a local word fails only with
    // `ShortCStrError::BadState`, the type's own bookkeeping invariant.
    // Chore #529: excluded from later respawns.
    // LESSONS: "Never hold a ForkCell borrow across a recursive call that re-borrows"
    Never,
}

impl core::error::Error for TimeoutError {}
