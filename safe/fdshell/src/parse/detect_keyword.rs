use super::Token;
use crate::error::parse::ParseError;
use crate::parse::line::ParsedLine;
use error_stack::{Report, ResultExt, bail};
use sys::ShortCStr;

pub(crate) fn detect_unset(tokens: &[Token]) -> Result<Option<ParsedLine>, Report<ParseError>> {
    let target = tokens
        .get(1)
        .ok_or(ParseError::ExpectedVariableNameAfterUnset)?;
    let Some(var) = target.0.strip_prefix(b"%") else {
        bail!(ParseError::VariableMustStartWithPercent)
    };
    if let Some((arr, source)) = super::array_ref::split_element_ref(&var) {
        return Ok(Some(ParsedLine::UnsetArrayEntry { var: arr, source }));
    }
    Ok(Some(ParsedLine::Unset(var)))
}

pub(crate) fn detect_umask(tokens: &[Token]) -> Result<Option<ParsedLine>, Report<ParseError>> {
    let mask = match tokens.get(1) {
        Some((arg, _, _, _, _)) => {
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

pub(crate) fn detect_control(tokens: &[Token]) -> Result<Option<ParsedLine>, Report<ParseError>> {
    let first = match tokens.first() {
        Some((t, _, _, _, _)) => t,
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

    if first.eq_bytes(b"return") {
        return detect_return(tokens);
    }

    Ok(None)
}

/// `return` with an optional integer status.
pub(crate) fn detect_return(tokens: &[Token]) -> Result<Option<ParsedLine>, Report<ParseError>> {
    if tokens.get(2).is_some() {
        bail!(ParseError::ReturnTakesAtMostOneArgument);
    }
    let code = match tokens.get(1) {
        None => None,
        Some((t, _, _, _, _)) => Some(parse_status(t)?),
    };
    Ok(Some(ParsedLine::Return(code)))
}

fn parse_status(t: &ShortCStr) -> Result<i32, Report<ParseError>> {
    t.parse::<i32>().change_context(ParseError::InvalidInt)
}
