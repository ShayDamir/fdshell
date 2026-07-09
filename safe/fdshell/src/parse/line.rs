use crate::parse::for_block::ForBlock;
use crate::parse::if_block::IfBlock;
use crate::parse::while_block::{UntilBlock, WhileBlock};
use crate::parse::{CommandLine, Pipeline};
use sys::ShortCStr;

pub enum ParsedLine {
    Cmd(CommandLine),
    Pipeline(Pipeline),
    AssignFd { var: ShortCStr, value: ShortCStr },
    AssignStr { var: ShortCStr, value: ShortCStr },
    Unset(ShortCStr),
    Umask(Option<u32>),
    If(IfBlock),
    For(ForBlock),
    While(WhileBlock),
    Until(UntilBlock),
    Break,
    Continue,
}
