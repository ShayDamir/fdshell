use crate::capture::Capture;
use crate::redirect::RedirectDef;
use sys::ShortCStr;

#[cfg_attr(test, derive(Debug, PartialEq))]
pub struct CommandLine {
    pub builtin: bool,
    pub command: ShortCStr,
    pub args: Vec<ShortCStr>,
    pub captures: Vec<Capture>,
    pub redirects: Vec<RedirectDef>,
    pub pidvar: Option<ShortCStr>,
    pub bg_force: bool,
}

#[cfg_attr(test, derive(Debug, PartialEq))]
pub struct Pipeline {
    pub commands: Vec<CommandLine>,
}
