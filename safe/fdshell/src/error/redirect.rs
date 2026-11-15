//! Redirection file opening errors (redirect/open.rs, redirect/resolve.rs).

/// [OpenRedirectError] Redirection file opening errors
#[derive(displaydoc::Display, Debug)]
pub(crate) enum OpenRedirectError {
    /// failed to open redirection path
    Open,
    /// fd variable '{var}' is not set
    VarNotFound { var: sys::ShortCStr },
    /// file descriptor number is out of range
    FdNumberOutOfRange,
    /// here-string: failed to expand the word
    HereStringExpand,
    /// here-string: failed to create the stdin file
    HereStringCreate,
    /// here-doc: failed to expand the body
    HereDocExpand,
    /// here-doc: failed to create the stdin file
    HereDocCreate,
    /// fd {n} is not open; `>&{n}` has nothing to duplicate
    FdNotOpen { n: i32 },
    /// cannot overwrite existing file '{name}': noclobber is set
    Noclobber { name: sys::ShortCStr },
    /// ambiguous redirect: {name}
    AmbiguousRedirect { name: sys::ShortCStr },
    /// strict mode forbids absolute redirect targets (use a path relative to
    /// the shell's working directory)
    StrictAbsoluteTarget { name: sys::ShortCStr },
    /// failed to expand the redirect target
    TargetExpand,
    /// failed to glob the redirect target
    Glob,
    /// failed to duplicate fd {n}; the descriptor table may be full
    DupFdFailed { n: i32 },
    /// failed to close redirection target fd {n}
    CloseFd { n: i32 },
    /// internal invariant violated
    // Truly never: the sites are ForkCell borrows, `as_bytes()` bookkeeping (only
    // `BadState`), `one_name`'s `first()` guarded by its own `len() == 1` arm,
    // `target.rs`'s `RedirectSource::Path` arm that callers route Path sources into,
    // and `resolve/arms.rs`'s `(None, None)` invariant — the open pass leaves an fd
    // for every ordinary def; only a magic entry has none.
    // LESSONS: "Never hold a ForkCell borrow across a recursive call that re-borrows"
    // Chore #565: excluded from later respawns.
    Never,
}

impl core::error::Error for OpenRedirectError {}
