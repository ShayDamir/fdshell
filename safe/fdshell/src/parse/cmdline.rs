use crate::capture::Capture;
use crate::redirect::RedirectDef;
use alloc::vec::Vec;
use sys::ShortCStr;

#[cfg_attr(test, derive(Debug, PartialEq))]
#[derive(Clone)]
pub struct CommandLine {
    pub builtin: bool,
    pub command: ShortCStr,
    pub args: Vec<ShortCStr>,
    /// Per-byte quote mask for each arg (parallel to `args`, each mask
    /// parallel to its arg). `true` marks bytes that were inside double
    /// quotes and are protected from IFS word splitting.
    pub args_mask: Vec<Vec<bool>>,
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
