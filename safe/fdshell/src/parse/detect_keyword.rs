use crate::error::parse::{ParseError, report_error};
use crate::parse::line::ParsedLine;
use error_stack::{Report, ResultExt, bail};
use sys::ShortCStr;

pub(crate) fn detect_unset(
    tokens: &[(ShortCStr, usize, bool)],
) -> Result<Option<ParsedLine>, Report<ParseError>> {
    let target = tokens
        .get(1)
        .ok_or_else(|| report_error("expected variable name after 'unset'", 0))?;
    if let Some(var) = target.0.strip_prefix(b"%") {
        return Ok(Some(ParsedLine::Unset(var)));
    }
    bail!(report_error("variable must start with '%'", 0))
}

pub(crate) fn detect_umask(
    tokens: &[(ShortCStr, usize, bool)],
) -> Result<Option<ParsedLine>, Report<ParseError>> {
    let mask = match tokens.get(1) {
        Some((arg, _, _)) => {
            let s = arg.as_bytes().change_context(ParseError::Reason {
                reason: "internal string state",
            })?;
            let s = core::str::from_utf8(s).change_context(ParseError::Reason {
                reason: "invalid UTF-8 bytes",
            })?;
            let s = s.strip_prefix("0o").unwrap_or(s);
            Some(
                u32::from_str_radix(s, 8).change_context(ParseError::Reason {
                    reason: "invalid octal number",
                })?,
            )
        }
        None => None,
    };
    if tokens.get(2).is_some() {
        bail!(report_error("umask takes at most one argument", 0));
    }
    Ok(Some(ParsedLine::Umask(mask)))
}

pub(crate) fn detect_control(
    tokens: &[(ShortCStr, usize, bool)],
) -> Result<Option<ParsedLine>, Report<ParseError>> {
    let first = match tokens.first() {
        Some((t, _, _)) => t,
        None => return Ok(None),
    };

    if first.eq_bytes(b"break") {
        if tokens.get(1).is_some() {
            bail!(report_error("break takes no arguments", 0));
        }
        return Ok(Some(ParsedLine::Break));
    }

    if first.eq_bytes(b"continue") {
        if tokens.get(1).is_some() {
            bail!(report_error("continue takes no arguments", 0));
        }
        return Ok(Some(ParsedLine::Continue));
    }

    Ok(None)
}
