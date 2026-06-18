use crate::error::parse::ParseError;
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
}

pub(crate) fn detect(tokens: &[ShortCStr]) -> Result<Option<ParsedLine>, ParseError> {
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
        let target = tokens.get(1).ok_or(ParseError::Reason {
            pos: 0,
            reason: "expected variable name after 'unset'",
        })?;
        if let Some(var) = target.strip_prefix(b"%") {
            return Ok(Some(ParsedLine::Unset(var)));
        }
        return Err(ParseError::Reason {
            pos: 0,
            reason: "variable must start with '%'",
        });
    }

    if first.eq_bytes(b"umask") {
        let mask = match tokens.get(1) {
            Some(arg) => {
                let s = arg
                    .as_bytes()
                    .map_err(|_| ParseError::InvalidChar { ch: 0, pos: 0 })?;
                let s = core::str::from_utf8(s)
                    .map_err(|_| ParseError::InvalidChar { ch: 0, pos: 0 })?;
                let s = s.strip_prefix("0o").unwrap_or(s);
                Some(u32::from_str_radix(s, 8).map_err(|_| ParseError::Reason {
                    pos: 0,
                    reason: "invalid octal number",
                })?)
            }
            None => None,
        };
        if tokens.get(2).is_some() {
            return Err(ParseError::Reason {
                pos: 0,
                reason: "umask takes at most one argument",
            });
        }
        return Ok(Some(ParsedLine::Umask(mask)));
    }

    Ok(None)
}
