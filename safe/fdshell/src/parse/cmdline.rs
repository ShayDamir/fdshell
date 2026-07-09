use crate::capture::Capture;
use crate::redirect::RedirectDef;
use sys::ShortCStr;

#[cfg_attr(test, derive(Debug, PartialEq))]
#[derive(Clone)]
pub struct CommandLine {
    pub builtin: bool,
    pub command: ShortCStr,
    pub args: Vec<ShortCStr>,
    /// `fully_quoted` flag for each arg (parallel to `args`).
    /// True if the arg was entirely within quotes (e.g., `"$@"` vs `$@`).
    pub args_fq: Vec<bool>,
    pub captures: Vec<Capture>,
    pub redirects: Vec<RedirectDef>,
    pub pidvar: Option<ShortCStr>,
    pub bg_force: bool,
}

#[cfg_attr(test, derive(Debug, PartialEq))]
#[derive(Clone)]
pub struct Pipeline {
    pub commands: Vec<CommandLine>,
}
