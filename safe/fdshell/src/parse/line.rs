use crate::parse::for_block::ForBlock;
use crate::parse::if_block::IfBlock;
use crate::parse::while_block::{UntilBlock, WhileBlock};
use crate::parse::{CommandLine, Pipeline};
use sys::ShortCStr;
use sys::errno::EINVAL;

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
}

pub(crate) fn detect(tokens: &[ShortCStr]) -> Result<Option<ParsedLine>, i32> {
    let first = match tokens.first() {
        Some(t) => t,
        None => return Ok(None),
    };

    if let Some((lhs, rhs)) = first.split_once_byte(b'=')
        && let Some(var) = lhs.strip_prefix(b"%")
        && let Some(value) = rhs.strip_prefix(b"%")
    {
        return Ok(Some(ParsedLine::AssignFd { var, value }));
    }

    if let Some((lhs, rhs)) = first.split_once_byte(b'=')
        && !lhs.is_empty()
        && !lhs.starts_with(b"%")
    {
        return Ok(Some(ParsedLine::AssignStr {
            var: lhs,
            value: rhs,
        }));
    }

    if first.eq_bytes(b"unset") {
        let target = tokens.get(1).ok_or(EINVAL)?;
        if let Some(var) = target.strip_prefix(b"%") {
            return Ok(Some(ParsedLine::Unset(var)));
        }
        return Err(EINVAL);
    }

    if first.eq_bytes(b"umask") {
        let mask = match tokens.get(1) {
            Some(arg) => {
                let s = core::str::from_utf8(arg.as_bytes()?).map_err(|_| EINVAL)?;
                let s = s.strip_prefix("0o").unwrap_or(s);
                Some(u32::from_str_radix(s, 8).map_err(|_| EINVAL)?)
            }
            None => None,
        };
        if tokens.get(2).is_some() {
            return Err(EINVAL);
        }
        return Ok(Some(ParsedLine::Umask(mask)));
    }

    Ok(None)
}
