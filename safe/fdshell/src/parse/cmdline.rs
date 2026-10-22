use crate::capture::Capture;
use crate::mask::MaskBit;
use crate::parse::Assign;
use crate::parse::ParsedLine;
use crate::redirect::RedirectDef;
use alloc::boxed::Box;
use alloc::vec::Vec;
use sys::ScriptText;
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
    /// Per-byte quote mask for the command word (word 0), parallel to it.
    /// Word 0 is expanded by `command_word` with this mask (substitution, IFS
    /// splitting, pathname expansion); after the expansion the child sees a
    /// fully-quoted mask, so its word-0 rules are no-ops on the name.
    pub command_mask: Vec<MaskBit>,
    /// Zero-byte quote-span indices of the command word, parallel to
    /// `command_mask` (task #321).
    pub command_span: Vec<usize>,
    /// Whether word 0's raw span contained quotes (word 0's counterpart of
    /// `args_quoted`, see `parse::word_quoted`): the quoted-empty rule —
    /// `"$(true)"` has no mask bit that says it, only the span does.
    pub command_quoted: bool,
    /// Leading `NAME=value` words scoped to this command (POSIX 2.9.1): raw
    /// value words, unexpanded; applied to the command's environment only.
    /// Each value carries its own per-byte quote mask — the word mask sliced
    /// at the `=` — so it is expanded with its own quoting (task #217).
    pub env_assigns: Vec<Assign>,
    pub args: Vec<ShortCStr>,
    /// Per-byte quote mask for each arg (parallel to `args`, each mask
    /// parallel to its arg). `true` marks bytes that were inside double
    /// quotes and are protected from IFS word splitting.
    pub args_mask: Vec<Vec<MaskBit>>,
    /// Zero-byte quote-span indices for each arg (task #321), parallel to
    /// `args_mask` and `args`.
    pub args_span: Vec<Vec<usize>>,
    /// Whether each arg's raw span contained quotes (parallel to `args`;
    /// see `parse::word_quoted`). True for a word like `""$(true)` whose
    /// quoted part expands to nothing — the mask alone cannot say.
    pub args_quoted: Vec<bool>,
    pub captures: Vec<Capture>,
    pub redirects: Vec<RedirectDef>,
    pub pidvar: Option<ShortCStr>,
    pub bg_force: bool,
}

/// One pipeline stage: a simple command, or a keyword block. A stage is a full
/// command (task #292), so the word that follows a `|` is in command position
/// and a keyword there is a block, not a word of the previous stage.
pub enum Stage {
    /// A simple command: word 0, args, redirects, captures — the pipeline's
    /// forked child execs it or runs the builtin.
    Command(CommandLine),
    /// A keyword block (`if`/`for`/`while`/`until`/`case`/event-`wait`/
    /// `name() { … }`) written as a stage: the child runs it through
    /// `run::run_parsed`, as bash runs a stage in its subshell.
    Block(BlockStage),
}

/// A keyword block written as a pipeline stage.
pub struct BlockStage {
    /// The block as the keyword parsers built it from the stage's tokens.
    pub parsed: ParsedLine,
    /// The block's own text (the statement sliced to the stage's token range):
    /// what `run_parsed` re-enters in the child.
    pub text: ScriptText,
}

impl Stage {
    /// The stage's simple command, `None` for a keyword block: a block has no
    /// word 0, no `NAME=value` prefix, no capture socket and no redirect list
    /// of its own, so every consumer that walks those lists skips it.
    pub fn command(&self) -> Option<&CommandLine> {
        match self {
            Self::Command(cmd) => Some(cmd),
            Self::Block(_) => None,
        }
    }
}

pub struct Pipeline {
    pub stages: Vec<Stage>,
}

impl Pipeline {
    /// The simple command of stage `i`, `None` when the index is out of range
    /// or the stage is a keyword block.
    pub fn command_at(&self, i: usize) -> Option<&CommandLine> {
        self.stages.get(i).and_then(Stage::command)
    }
}

/// A stage after the parent's word-0 expansion (`command_word::expand_pipeline`):
/// the expanded simple command — word 0 may have become a command name plus
/// leading arguments, so the expansion owns it, boxed to keep the two variants
/// the same size — or the keyword block, which has no word 0 to expand and
/// travels by reference (task #292).
pub enum ExpandedStage<'a> {
    Command(Box<CommandLine>),
    Block(&'a BlockStage),
}

impl<'a> ExpandedStage<'a> {
    /// The stage's simple command, `None` for a keyword block.
    pub fn command(&self) -> Option<&CommandLine> {
        match self {
            Self::Command(cmd) => Some(cmd),
            Self::Block(_) => None,
        }
    }

    /// The stage's keyword block, `None` for a simple command.
    pub fn block(&self) -> Option<&'a BlockStage> {
        match self {
            Self::Block(block) => Some(block),
            Self::Command(_) => None,
        }
    }

    /// The stage's capture operators (`>&%name`): empty for a keyword block,
    /// which has no capture socket — bash has no capture syntax on a compound.
    pub fn captures(&self) -> &[Capture] {
        self.command().map_or(&[], |cmd| cmd.captures.as_slice())
    }

    /// The redirections written inside the stage. A keyword block carries its
    /// own redirects in its parsed arms: a redirect written after the block's
    /// closer is not applied to the block (the accepted divergence of this
    /// family, README "Pipelines and lists").
    pub fn redirects(&self) -> &[RedirectDef] {
        self.command().map_or(&[], |cmd| cmd.redirects.as_slice())
    }
}
