use crate::error::parse::ParseError;
use crate::error::parse::{report_error, report_invalid_char};
use crate::parse::for_block::ForBlock;
use crate::parse::if_block::IfBlock;
use crate::parse::while_block::{UntilBlock, WhileBlock};
use crate::parse::{CommandLine, Pipeline};
use error_stack::Report;
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

pub(crate) fn detect(
    tokens: &[(ShortCStr, usize)],
) -> Result<Option<ParsedLine>, Report<ParseError>> {
    let first = match tokens.first() {
        Some((t, _)) => t,
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
        let target = tokens
            .get(1)
            .ok_or_else(|| report_error("expected variable name after 'unset'", 0))?;
        if let Some(var) = target.0.strip_prefix(b"%") {
            return Ok(Some(ParsedLine::Unset(var)));
        }
        return Err(report_error("variable must start with '%'", 0));
    }

    if first.eq_bytes(b"umask") {
        let mask = match tokens.get(1) {
            Some((arg, _)) => {
                let s = arg.as_bytes().map_err(|_| report_invalid_char(0, 0))?;
                let s = core::str::from_utf8(s).map_err(|_| report_invalid_char(0, 0))?;
                let s = s.strip_prefix("0o").unwrap_or(s);
                Some(
                    u32::from_str_radix(s, 8)
                        .map_err(|_| report_error("invalid octal number", 0))?,
                )
            }
            None => None,
        };
        if tokens.get(2).is_some() {
            return Err(report_error("umask takes at most one argument", 0));
        }
        return Ok(Some(ParsedLine::Umask(mask)));
    }

    Ok(None)
}
