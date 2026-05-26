use crate::capture::Capture;
use crate::redirect::Redirect;
use sys::ShortCStr;

pub struct CommandLine {
    pub builtin: bool,
    pub command: ShortCStr,
    pub args: Vec<ShortCStr>,
    pub captures: Vec<Capture>,
    pub redirects: Vec<Redirect>,
    pub background: bool,
}

impl PartialEq for CommandLine {
    fn eq(&self, other: &Self) -> bool {
        self.builtin == other.builtin
            && self.command == other.command
            && self.args == other.args
            && self.captures == other.captures
            && self.redirects == other.redirects
            && self.background == other.background
    }
}

impl core::fmt::Debug for CommandLine {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("CommandLine")
            .field("builtin", &self.builtin)
            .field("command", &self.command)
            .field("args", &self.args)
            .field("captures", &self.captures)
            .field("redirects", &self.redirects)
            .field("background", &self.background)
            .finish()
    }
}
