use super::CommandLine;
use crate::capture::Capture;
use crate::parse::ParsedLine;
use crate::redirect::RedirectDef;
use alloc::boxed::Box;
use alloc::vec::Vec;
use sys::ScriptText;

/// One pipeline stage: a simple command, or a keyword block. A stage is a full
/// command (task #292), so the word that follows a `|` is in command position
/// and a keyword there is a block, not a word of the previous stage. Both
/// variants carry the stage's **own text slice** (task #481): the run layer hands
/// it to the dispatch decision, which is where a function name must be resolved
/// (the statement path's function arm is `run/parent.rs`).
pub enum Stage {
    /// A simple command: word 0, args, redirects, captures — the pipeline's
    /// forked child execs it or runs the builtin — and the stage's own text.
    Command(CommandLine, ScriptText),
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
            Self::Command(cmd, _) => Some(cmd),
            Self::Block(_) => None,
        }
    }

    /// The stage's own text slice — see [`Stage`].
    pub fn text(&self) -> &ScriptText {
        match self {
            Self::Command(_, text) => text,
            Self::Block(block) => &block.text,
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
    Command(Box<CommandLine>, ScriptText),
    Block(&'a BlockStage),
}

impl<'a> ExpandedStage<'a> {
    /// The stage's simple command, `None` for a keyword block.
    pub fn command(&self) -> Option<&CommandLine> {
        match self {
            Self::Command(cmd, _) => Some(cmd),
            Self::Block(_) => None,
        }
    }

    /// The stage's own text slice — the text the forked child's dispatch reads
    /// a function name at (task #481), the parse layer's slice of the statement
    /// to this stage's tokens.
    pub fn text(&self) -> &ScriptText {
        match self {
            Self::Command(_, text) => text,
            Self::Block(block) => &block.text,
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
