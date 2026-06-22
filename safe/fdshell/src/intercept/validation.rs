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

pub(crate) fn check_builtin_not_supported(
    line: &[u8],
    command: &'static str,
    builtin: bool,
) -> Result<(), Report<CmdError>> {
    if builtin {
        let pos = line.windows(7).position(|w| w == b"builtin").unwrap_or(0);
        return Err(err_at(
            line,
            pos,
            CmdError::BuiltinKeywordNotSupported { command },
        ));
    }
    Ok(())
}

pub(crate) fn check_captures_not_supported(
    line: &[u8],
    command: &'static str,
    captures: &[Capture],
) -> Result<(), Report<CmdError>> {
    if !captures.is_empty() {
        let pos = line.windows(2).position(|w| w == b"%>").unwrap_or(0);
        return Err(err_at(
            line,
            pos,
            CmdError::CapturesNotSupported { command },
        ));
    }
    Ok(())
}

pub(crate) fn check_redirects_not_supported(
    line: &[u8],
    command: &'static str,
    redirects: &[RedirectDef],
) -> Result<(), Report<CmdError>> {
    if !redirects.is_empty() {
        let pos = line
            .iter()
            .position(|&b| b == b'<' || b == b'>')
            .unwrap_or(0);
        return Err(err_at(
            line,
            pos,
            CmdError::RedirectNotSupported { command },
        ));
    }
    Ok(())
}
