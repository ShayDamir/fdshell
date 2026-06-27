/// Top-level error for fdshell
#[derive(Debug, displaydoc::Display)]
pub enum AppError {
    /// initialization failed
    Init,
    /// command execution failed
    Exec,
    /// CWD directory open failed
    Cwd,
    /// input read error
    Read,
    /// state borrow failed
    Borrow,
    /// usage: fdshell [-c cmd] [name args...] or fdshell [--dirfd fd|--fd fd] script [args...]
    Usage,
    /// missing value for --{0}
    MissingValue(&'static str),
    /// invalid fd number for --{0}
    InvalidFdNumber(&'static str),
    /// failed to set CLOEXEC on --fd
    CloexecFailed,
    /// failed to read script file
    ScriptRead,
    /// missing script path after --dirfd
    MissingScriptPath,
}

impl core::error::Error for AppError {}
