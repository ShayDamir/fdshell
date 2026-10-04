use crate::capture::Capture;
use crate::redirect::RedirectDef;
use alloc::vec::Vec;
use sys::ShortCStr;

#[cfg_attr(test, derive(Debug, PartialEq))]
#[derive(Clone)]
pub struct CommandLine {
    pub builtin: bool,
    pub command: ShortCStr,
    /// Per-byte quote mask for the command word (word 0), parallel to it;
    /// its unquoted pattern bytes glob the command name (bash field model).
    pub command_mask: Vec<bool>,
    /// Leading `NAME=value` words scoped to this command (POSIX 2.9.1): raw
    /// value words, unexpanded; applied to the command's environment only.
    pub env_assigns: Vec<(ShortCStr, ShortCStr)>,
    pub args: Vec<ShortCStr>,
    /// Per-byte quote mask for each arg (parallel to `args`, each mask
    /// parallel to its arg). `true` marks bytes that were inside double
    /// quotes and are protected from IFS word splitting.
    pub args_mask: Vec<Vec<bool>>,
    /// Whether each arg's raw span contained quotes (parallel to `args`;
    /// see `parse::word_quoted`). True for a word like `""$(true)` whose
    /// quoted part expands to nothing — the mask alone cannot say.
    pub args_quoted: Vec<bool>,
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
