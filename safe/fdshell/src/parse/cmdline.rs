use crate::capture::Capture;
use crate::redirect::RedirectDef;
use alloc::vec::Vec;
use sys::ShortCStr;

/// How a command name is dispatched, set by a leading `builtin`/`command`
/// keyword (or a bare name the parser already knows to be a builtin).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuiltinPrefix {
    /// No keyword; normal resolution (function, intercept, builtin, external).
    None,
    /// `builtin NAME`, or a bare known-builtin name: dispatch as a builtin
    /// only; a non-builtin is an error.
    Builtin,
    /// `command NAME`: dispatch as a builtin if the name is one, otherwise
    /// fall through to the external (PATH) search.
    Command,
}

#[cfg_attr(test, derive(Debug, PartialEq))]
#[derive(Clone)]
pub struct CommandLine {
    /// Dispatch prefix for the command word (`builtin`/`command` keyword).
    pub prefix: BuiltinPrefix,
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
