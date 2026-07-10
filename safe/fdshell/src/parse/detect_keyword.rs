use crate::error::parse::ParseError;
use crate::parse::line::ParsedLine;
use error_stack::{Report, ResultExt, bail};
use sys::ShortCStr;

pub(crate) fn detect_unset(
    tokens: &[(ShortCStr, usize, bool)],
) -> Result<Option<ParsedLine>, Report<ParseError>> {
    let target = tokens
        .get(1)
        .ok_or(ParseError::ExpectedVariableNameAfterUnset)?;
    if let Some(var) = target.0.strip_prefix(b"%") {
        return Ok(Some(ParsedLine::Unset(var)));
    }
    bail!(ParseError::VariableMustStartWithPercent)
}

pub(crate) fn detect_umask(
    tokens: &[(ShortCStr, usize, bool)],
) -> Result<Option<ParsedLine>, Report<ParseError>> {
    let mask = match tokens.get(1) {
        Some((arg, _, _)) => {
            let s = arg.as_bytes().change_context(ParseError::Never)?;
            let s = core::str::from_utf8(s).change_context(ParseError::InvalidChar { ch: 0 })?;
            let s = s.strip_prefix("0o").unwrap_or(s);
            Some(u32::from_str_radix(s, 8).change_context(ParseError::InvalidOctal)?)
        }
        None => None,
    };
    if tokens.get(2).is_some() {
        bail!(ParseError::UmaskTakesAtMostOneArgument);
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
            bail!(ParseError::BreakTakesNoArguments);
        }
        return Ok(Some(ParsedLine::Break));
    }

    if first.eq_bytes(b"continue") {
        if tokens.get(1).is_some() {
            bail!(ParseError::ContinueTakesNoArguments);
        }
        return Ok(Some(ParsedLine::Continue));
    }

    Ok(None)
}
