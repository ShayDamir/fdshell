//! Command dispatch errors (run.rs, cond.rs).

/// [CmdError] Command dispatch errors
#[derive(displaydoc::Display, Debug)]
pub enum CmdError {
    /// invalid command
    Invalid,
    /// exit: invalid argument
    ExitArgInvalid,
    /// {command}: captures are not supported
    CapturesNotSupported { command: &'static str },
    /// {command}: redirects are not supported
    RedirectNotSupported { command: &'static str },
    /// {command}: `builtin` prefix is not supported
    BuiltinKeywordNotSupported { command: &'static str },
    /// parse error
    Parse,
    /// launch failed
    Launch,
    /// capture failed
    Capture,
    /// pipeline failed
    Pipeline,
    /// redirection failed
    Redirect,
    /// resolution error
    Resolve,
    /// execution error
    Exec,
    /// cd failed
    Cd,
    /// invalid export name
    ExportName,
    /// fd pass-through failed
    FdPass,
    /// command substitution failed
    CmdSubst,
    /// task management failed
    Task,
    /// envfilter: missing arguments (try --help)
    EnvfilterNoArgs,
    /// envfilter: unknown flag
    EnvfilterUnknownFlag,
    /// {0} requires a pattern
    PatternRequired(&'static str),
}

impl core::error::Error for CmdError {}
