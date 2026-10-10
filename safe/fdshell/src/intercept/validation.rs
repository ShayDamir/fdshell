use error_stack::Report;

use crate::capture::Capture;
use crate::error::cmd::CmdError;
use crate::error::parse::ParsePosition;
use crate::parse::BuiltinPrefix;

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

/// Reject the capture (`%>`) extra an intercepted builtin does not support. A
/// capture needs a forked child to send the fd over the shell socket
/// (`capture.rs::do_captures` reads from the child-end socketpair), so it stays
/// a genuine design limit for in-process commands; their redirections are
/// applied by `redirect::Scope` in `run/parent.rs`.
pub(crate) fn check_captures_not_supported(
    line: &[u8],
    command: &'static str,
    captures: &[Capture],
) -> Result<(), Report<CmdError>> {
    if captures.is_empty() {
        return Ok(());
    }
    let pos = line.windows(2).position(is_capture).unwrap_or(0);
    reject_at(line, pos, CmdError::CapturesNotSupported { command })
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

/// The same check without the `builtin`/`command` keyword arm. The split is
/// load-bearing: `builtin local`/`builtin let`/`builtin times`/`builtin wait`/
/// `builtin export_fd` are accepted, `builtin cd`/`builtin eval` are rejected.
pub(crate) fn validate_intercept_no_builtin(
    line: &[u8],
    command: &'static str,
    cmdline: &crate::parse::CommandLine,
) -> Result<(), Report<CmdError>> {
    check_captures_not_supported(line, command, &cmdline.captures)
}
