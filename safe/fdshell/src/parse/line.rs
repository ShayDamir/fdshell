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
    Unset(ShortCStr),
    UnsetArrayEntry {
        var: ShortCStr,
        source: ShortCStr,
    },
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
