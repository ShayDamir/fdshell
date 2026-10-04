use crate::parse::case_block::CaseBlock;
use crate::parse::for_block::ForBlock;
use crate::parse::function_block::FunctionDef;
use crate::parse::if_block::IfBlock;
use crate::parse::wait_block::WaitBlock;
use crate::parse::while_block::{UntilBlock, WhileBlock};
use crate::parse::{CommandLine, Pipeline};
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
    AssignStr {
        var: ShortCStr,
        value: ShortCStr,
    },
    /// A bare statement of several `NAME=value` words; all persist.
    AssignStrs {
        assigns: Vec<(ShortCStr, ShortCStr)>,
    },
    Unset(ShortCStr),
    UnsetArrayEntry {
        var: ShortCStr,
        source: ShortCStr,
    },
    Umask(Option<u32>),
    Case(CaseBlock),
    If(IfBlock),
    Wait(WaitBlock),
    For(ForBlock),
    While(WhileBlock),
    Until(UntilBlock),
    Function(FunctionDef),
    Break,
    Continue,
    Return(Option<i32>),
}
