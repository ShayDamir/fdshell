//! Launch / child execution errors (launch.rs, child/*.rs).

/// [LaunchError] Launch / child execution errors
#[derive(displaydoc::Display, Debug)]
pub(crate) enum LaunchError {
    /// fork syscall failed
    Fork,
    /// child exec syscall failed
    Exec,
    /// builtin dispatch in child failed
    BuiltinDispatch,
    /// redirect file open failed
    Redirect,
    /// capture socketpair creation failed
    CaptureSocket,
    /// state borrow failed
    Borrow,
    /// failed to receive the last argument from the child
    LastArg,
    /// impossible error state (should never occur)
    // Truly never: `last_arg.rs`'s tag check only ever sees the child's own `$_`
    // message — it is the first message on the socket, a builtin sending captures
    // runs only after the exec — and its `get(..n)` bound is a read count bounded by
    // the buffer (§4.11). Chore #516: excluded from later respawns.
    Never,
}

impl core::error::Error for LaunchError {}
