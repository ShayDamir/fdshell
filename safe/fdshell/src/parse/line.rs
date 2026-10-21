use crate::parse::case_block::CaseBlock;
use crate::parse::for_block::ForBlock;
use crate::parse::function_block::FunctionDef;
use crate::parse::group_block::GroupBlock;
use crate::parse::if_block::IfBlock;
use crate::parse::time_block::TimeBlock;
use crate::parse::wait_block::WaitBlock;
use crate::parse::while_block::{UntilBlock, WhileBlock};
use crate::parse::{Assign, CommandLine, Pipeline};
use alloc::vec::Vec;
use sys::ScriptText;
use sys::ShortCStr;

pub enum ParsedLine {
    Cmd(CommandLine),
    Pipeline(Pipeline),
    /// The `((expr))` arithmetic command: the raw expression body.
    ArithCommand(ScriptText),
    AssignFd {
        var: ShortCStr,
        value: ShortCStr,
    },
    AssignFdIndex {
        var: ShortCStr,
        value: ShortCStr,
        index: usize,
    },
    AssignArrayEmpty {
        var: ShortCStr,
    },
    AppendFd {
        var: ShortCStr,
        value: ShortCStr,
    },
    /// A bare `NAME=value` statement: the raw word with its value's quote mask.
    AssignStr(Assign),
    /// A bare statement of several `NAME=value` words; all persist.
    AssignStrs(Vec<Assign>),
    /// `unset name…`: every operand word, in the order it was written.
    Unset(Vec<UnsetTarget>),
    Umask(Option<u32>),
    /// The `{ …; }` group command: the body text and the redirections written
    /// after the closing brace.
    Group(GroupBlock),
    Case(CaseBlock),
    If(IfBlock),
    Wait(WaitBlock),
    For(ForBlock),
    While(WhileBlock),
    Until(UntilBlock),
    Function(FunctionDef),
    /// `[-p] <statement>`: report the statement's real/user/sys times on
    /// stderr, with the statement's exit status.
    Time(TimeBlock),
    /// `break n`: exit `n` enclosing loops, `n >= 1`.
    Break(u32),
    /// `continue n`: advance `n` enclosing loops, `n >= 1`.
    Continue(u32),
    Return(Option<i32>),
}

/// One `unset` operand (task #290). The `%` prefix selects fdshell's fd
/// namespace, as everywhere else (STYLE.md §5); a plain word is a shell
/// variable name, the spelling bash's `unset name…` takes.
pub enum UnsetTarget {
    /// `%name` with the `%` stripped: the fd variable — the descriptor, an fd
    /// array of that name, and a named background task.
    Fd(ShortCStr),
    /// `%arr[%src]`: one entry of the fd array `arr` — the one whose provenance
    /// is `%src` — leaving the array in place.
    FdArrayEntry { var: ShortCStr, source: ShortCStr },
    /// `name`: a shell variable, taken out of every store that holds it.
    Var(ShortCStr),
}
