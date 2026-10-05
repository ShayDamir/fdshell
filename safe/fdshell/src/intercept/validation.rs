use error_stack::Report;

use crate::capture::Capture;
use crate::error::cmd::CmdError;
use crate::error::parse::ParsePosition;
use crate::parse::BuiltinPrefix;
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
    prefix: BuiltinPrefix,
) -> Result<(), Report<CmdError>> {
    if prefix == BuiltinPrefix::None {
        return Ok(());
    }
    let pos = line.windows(7).position(is_builtin_kw).unwrap_or(0);
    reject_at(line, pos, CmdError::BuiltinKeywordNotSupported { command })
}

/// Reject the capture (`%>`) and redirection extras an intercepted builtin does
/// not support; captures are reported first when both are present.
pub(crate) fn check_extras_not_supported(
    line: &[u8],
    command: &'static str,
    captures: &[Capture],
    redirects: &[RedirectDef],
) -> Result<(), Report<CmdError>> {
    if !captures.is_empty() {
        let pos = line.windows(2).position(is_capture).unwrap_or(0);
        return reject_at(line, pos, CmdError::CapturesNotSupported { command });
    }
    if !redirects.is_empty() {
        let pos = line.iter().position(is_redirect).unwrap_or(0);
        return reject_at(line, pos, CmdError::RedirectNotSupported { command });
    }
    Ok(())
}

fn reject_at(line: &[u8], pos: usize, err: CmdError) -> Result<(), Report<CmdError>> {
    Err(err_at(line, pos, err))
}

pub(crate) fn validate_intercept(
    line: &[u8],
    command: &'static str,
    cmdline: &crate::parse::CommandLine,
) -> Result<(), Report<CmdError>> {
    check_builtin_not_supported(line, command, cmdline.prefix)?;
    validate_intercept_no_builtin(line, command, cmdline)
}

pub(crate) fn validate_intercept_no_builtin(
    line: &[u8],
    command: &'static str,
    cmdline: &crate::parse::CommandLine,
) -> Result<(), Report<CmdError>> {
    check_extras_not_supported(line, command, &cmdline.captures, &cmdline.redirects)
}
