use error_stack::Report;

use crate::capture::Capture;
use crate::error::cmd::CmdError;
use crate::error::parse::ParsePosition;
use crate::redirect::RedirectDef;

pub(crate) fn err_at(line: &[u8], pos: usize, err: CmdError) -> Report<CmdError> {
    Report::new(err).attach_opaque(ParsePosition {
        pos,
        input: Some(line.to_vec()),
    })
}

fn is_builtin_kw(w: &[u8]) -> bool {
    w == b"builtin" || w == b"command"
}

fn is_capture(w: &[u8]) -> bool {
    w == b"%>"
}

fn is_redirect(b: &u8) -> bool {
    *b == b'<' || *b == b'>'
}

pub(crate) fn check_builtin_not_supported(
    line: &[u8],
    command: &'static str,
    builtin: bool,
) -> Result<(), Report<CmdError>> {
    if !builtin {
        return Ok(());
    }
    let pos = line.windows(7).position(is_builtin_kw).unwrap_or(0);
    Err(err_at(
        line,
        pos,
        CmdError::BuiltinKeywordNotSupported { command },
    ))
}

pub(crate) fn check_captures_not_supported(
    line: &[u8],
    command: &'static str,
    captures: &[Capture],
) -> Result<(), Report<CmdError>> {
    if captures.is_empty() {
        return Ok(());
    }
    let pos = line.windows(2).position(is_capture).unwrap_or(0);
    Err(err_at(
        line,
        pos,
        CmdError::CapturesNotSupported { command },
    ))
}

pub(crate) fn check_redirects_not_supported(
    line: &[u8],
    command: &'static str,
    redirects: &[RedirectDef],
) -> Result<(), Report<CmdError>> {
    if redirects.is_empty() {
        return Ok(());
    }
    let pos = line.iter().position(is_redirect).unwrap_or(0);
    Err(err_at(
        line,
        pos,
        CmdError::RedirectNotSupported { command },
    ))
}

pub(crate) fn validate_intercept(
    line: &[u8],
    command: &'static str,
    cmdline: &crate::parse::CommandLine,
) -> Result<(), Report<CmdError>> {
    check_builtin_not_supported(line, command, cmdline.builtin)?;
    validate_intercept_no_builtin(line, command, cmdline)
}

pub(crate) fn validate_intercept_no_builtin(
    line: &[u8],
    command: &'static str,
    cmdline: &crate::parse::CommandLine,
) -> Result<(), Report<CmdError>> {
    check_captures_not_supported(line, command, &cmdline.captures)?;
    check_redirects_not_supported(line, command, &cmdline.redirects)?;
    Ok(())
}
