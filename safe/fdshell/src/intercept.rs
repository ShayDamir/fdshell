use error_stack::{Report, ResultExt};

use crate::error::cmd::CmdError;
use crate::error::parse::ParsePosition;
use crate::state::ShellState;
use sys::fork_cell::ForkCell;
use sys::siginfo::WaitStatus;

fn err_at(line: &[u8], pos: usize, err: CmdError) -> Report<CmdError> {
    Report::new(err).attach_opaque(ParsePosition {
        pos,
        input: Some(line.to_vec()),
    })
}

pub(crate) fn try_intercept(
    line: &[u8],
    cmdline: &crate::parse::CommandLine,
    cell: &ForkCell<ShellState>,
) -> Result<bool, Report<CmdError>> {
    let mut state = cell.borrow_mut().change_context(CmdError::Exec)?;
    match cmdline.command.as_bytes().change_context(CmdError::Exec)? {
        b"cd" => {
            if cmdline.builtin {
                let pos = line.windows(7).position(|w| w == b"builtin").unwrap_or(0);
                return Err(err_at(
                    line,
                    pos,
                    CmdError::BuiltinKeywordNotSupported { command: "cd" },
                ));
            }
            if !cmdline.captures.is_empty() {
                let pos = line.windows(2).position(|w| w == b"%>").unwrap_or(0);
                return Err(err_at(
                    line,
                    pos,
                    CmdError::CapturesNotSupported { command: "cd" },
                ));
            }
            if !cmdline.redirects.is_empty() {
                let pos = line
                    .iter()
                    .position(|&b| b == b'<' || b == b'>')
                    .unwrap_or(0);
                return Err(err_at(
                    line,
                    pos,
                    CmdError::RedirectNotSupported { command: "cd" },
                ));
            }
            crate::cd::cd(&cmdline.args, &mut state).change_context(CmdError::Cd)?;
            state.last_status = WaitStatus::Exited(0);
        }
        b"exit" | b"quit" => {
            if cmdline.builtin {
                let pos = line.windows(7).position(|w| w == b"builtin").unwrap_or(0);
                return Err(err_at(
                    line,
                    pos,
                    CmdError::BuiltinKeywordNotSupported { command: "exit" },
                ));
            }
            if !cmdline.captures.is_empty() {
                let pos = line.windows(2).position(|w| w == b"%>").unwrap_or(0);
                return Err(err_at(
                    line,
                    pos,
                    CmdError::CapturesNotSupported { command: "exit" },
                ));
            }
            if !cmdline.redirects.is_empty() {
                let pos = line
                    .iter()
                    .position(|&b| b == b'<' || b == b'>')
                    .unwrap_or(0);
                return Err(err_at(
                    line,
                    pos,
                    CmdError::RedirectNotSupported { command: "exit" },
                ));
            }
            let code = match cmdline.args.first() {
                Some(arg) => {
                    let s = core::str::from_utf8(arg.as_bytes().change_context(CmdError::Exec)?)
                        .change_context(CmdError::ExitArgInvalid)?;
                    s.parse::<i32>().change_context(CmdError::ExitArgInvalid)?
                }
                None => state.last_status.exit_code(),
            };
            std::process::exit(code);
        }
        b"become" => {
            if !cmdline.captures.is_empty() {
                eprintln!("become: captures not supported");
                std::process::exit(sys::errno::EINVAL);
            }
            drop(state);
            match crate::replacer::execute(&cmdline.args, &cmdline.redirects, cell) {
                Ok(code) => std::process::exit(code),
                Err(report) => {
                    eprintln!("{:?}", report);
                    std::process::exit(report.current_context().exit_code());
                }
            }
        }
        b"export_fd" if cmdline.builtin => {
            if !cmdline.captures.is_empty() {
                let pos = line.windows(2).position(|w| w == b"%>").unwrap_or(0);
                return Err(err_at(
                    line,
                    pos,
                    CmdError::CapturesNotSupported {
                        command: "export_fd",
                    },
                ));
            }
            if !cmdline.redirects.is_empty() {
                let pos = line
                    .iter()
                    .position(|&b| b == b'<' || b == b'>')
                    .unwrap_or(0);
                return Err(err_at(
                    line,
                    pos,
                    CmdError::RedirectNotSupported {
                        command: "export_fd",
                    },
                ));
            }
            state.last_status = match crate::child::fdpass::export_fd(&cmdline.args, &state) {
                Ok(_) => WaitStatus::Exited(0),
                Err(report) => WaitStatus::Exited(match report.current_context() {
                    crate::error::fdpass::FdPassError::SendFailed => sys::errno::EIO,
                    crate::error::fdpass::FdPassError::NotFound
                    | crate::error::fdpass::FdPassError::InvalidName
                    | crate::error::fdpass::FdPassError::MissingArg => sys::errno::EINVAL,
                }),
            };
        }
        b"wait" => {
            if cmdline.builtin {
                let pos = line.windows(7).position(|w| w == b"builtin").unwrap_or(0);
                return Err(err_at(
                    line,
                    pos,
                    CmdError::BuiltinKeywordNotSupported { command: "wait" },
                ));
            }
            if !cmdline.captures.is_empty() {
                let pos = line.windows(2).position(|w| w == b"%>").unwrap_or(0);
                return Err(err_at(
                    line,
                    pos,
                    CmdError::CapturesNotSupported { command: "wait" },
                ));
            }
            if !cmdline.redirects.is_empty() {
                let pos = line
                    .iter()
                    .position(|&b| b == b'<' || b == b'>')
                    .unwrap_or(0);
                return Err(err_at(
                    line,
                    pos,
                    CmdError::RedirectNotSupported { command: "wait" },
                ));
            }
            state.last_status =
                crate::task::try_wait(&cmdline.args, &mut state).change_context(CmdError::Task)?;
        }
        b"export" => {
            if cmdline.builtin {
                let pos = line.windows(7).position(|w| w == b"builtin").unwrap_or(0);
                return Err(err_at(
                    line,
                    pos,
                    CmdError::BuiltinKeywordNotSupported { command: "export" },
                ));
            }
            if !cmdline.captures.is_empty() {
                let pos = line.windows(2).position(|w| w == b"%>").unwrap_or(0);
                return Err(err_at(
                    line,
                    pos,
                    CmdError::CapturesNotSupported { command: "export" },
                ));
            }
            if !cmdline.redirects.is_empty() {
                let pos = line
                    .iter()
                    .position(|&b| b == b'<' || b == b'>')
                    .unwrap_or(0);
                return Err(err_at(
                    line,
                    pos,
                    CmdError::RedirectNotSupported { command: "export" },
                ));
            }
            crate::exports::handle_export(&cmdline.args, &mut state)
                .change_context(CmdError::ExportName)?;
            state.last_status = WaitStatus::Exited(0);
        }
        _ => return Ok(false),
    }
    Ok(true)
}
