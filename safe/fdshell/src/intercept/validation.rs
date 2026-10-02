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

/// `Ok` when `present` is false; otherwise the `make_err` variant at the
/// first position `find_pos` reports on the line (0 when none).
fn not_supported(
    line: &[u8],
    command: &'static str,
    present: bool,
    find_pos: impl Fn(&[u8]) -> Option<usize>,
    make_err: impl FnOnce(&'static str) -> CmdError,
) -> Result<(), Report<CmdError>> {
    if !present {
        return Ok(());
    }
    Err(err_at(line, find_pos(line).unwrap_or(0), make_err(command)))
}

pub(crate) fn check_builtin_not_supported(
    line: &[u8],
    command: &'static str,
    builtin: bool,
) -> Result<(), Report<CmdError>> {
    not_supported(
        line,
        command,
        builtin,
        |l| l.windows(7).position(|w| w == b"builtin" || w == b"command"),
        |c| CmdError::BuiltinKeywordNotSupported { command: c },
    )
}

pub(crate) fn check_captures_not_supported(
    line: &[u8],
    command: &'static str,
    captures: &[Capture],
) -> Result<(), Report<CmdError>> {
    not_supported(
        line,
        command,
        !captures.is_empty(),
        |l| l.windows(2).position(|w| w == b"%>"),
        |c| CmdError::CapturesNotSupported { command: c },
    )
}

pub(crate) fn check_redirects_not_supported(
    line: &[u8],
    command: &'static str,
    redirects: &[RedirectDef],
) -> Result<(), Report<CmdError>> {
    not_supported(
        line,
        command,
        !redirects.is_empty(),
        |l| l.iter().position(|&b| b == b'<' || b == b'>'),
        |c| CmdError::RedirectNotSupported { command: c },
    )
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
