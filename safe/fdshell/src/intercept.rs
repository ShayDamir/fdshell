use error_stack::{Report, ResultExt, bail};

use crate::error::cmd::CmdError;
use crate::state::ShellState;
use sys::fork_cell::ForkCell;
use sys::siginfo::WaitStatus;

pub(crate) fn try_intercept(
    cmdline: &crate::parse::CommandLine,
    cell: &ForkCell<ShellState>,
) -> Result<bool, Report<CmdError>> {
    let mut state = cell.borrow_mut().map_err(|_| CmdError::Exec)?;
    match cmdline.command.as_bytes().map_err(|_| CmdError::Exec)? {
        b"cd" => {
            if cmdline.builtin {
                bail!(CmdError::BuiltinKeywordNotSupported { command: "cd" });
            }
            if !cmdline.captures.is_empty() {
                bail!(CmdError::CapturesNotSupported { command: "cd" });
            }
            if !cmdline.redirects.is_empty() {
                bail!(CmdError::RedirectNotSupported { command: "cd" });
            }
            crate::cd::cd(&cmdline.args, &mut state)?;
            state.last_status = WaitStatus::Exited(0);
        }
        b"exit" | b"quit" => {
            if cmdline.builtin {
                bail!(CmdError::BuiltinKeywordNotSupported { command: "exit" });
            }
            if !cmdline.captures.is_empty() {
                bail!(CmdError::CapturesNotSupported { command: "exit" });
            }
            if !cmdline.redirects.is_empty() {
                bail!(CmdError::RedirectNotSupported { command: "exit" });
            }
            let code = match cmdline.args.first() {
                Some(arg) => {
                    let s = core::str::from_utf8(arg.as_bytes().map_err(|_| CmdError::Exec)?)
                        .change_context(CmdError::Builtin)?;
                    s.parse::<i32>().change_context(CmdError::Builtin)?
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
            crate::replacer::replace_shell(&cmdline.args, &cmdline.redirects, cell);
        }
        b"export_fd" if cmdline.builtin => {
            if !cmdline.captures.is_empty() {
                bail!(CmdError::CapturesNotSupported {
                    command: "export_fd"
                });
            }
            if !cmdline.redirects.is_empty() {
                bail!(CmdError::RedirectNotSupported {
                    command: "export_fd"
                });
            }
            state.last_status = match crate::child::fdpass::export_fd(&cmdline.args, &state) {
                Ok(()) => WaitStatus::Exited(0),
                Err(e) => WaitStatus::Exited(match e {
                    crate::error::fdpass::FdPassError::SendFailed => sys::errno::EIO,
                    crate::error::fdpass::FdPassError::NotFound
                    | crate::error::fdpass::FdPassError::InvalidName
                    | crate::error::fdpass::FdPassError::MissingArg => sys::errno::EINVAL,
                }),
            };
        }
        b"wait" => {
            if cmdline.builtin {
                bail!(CmdError::BuiltinKeywordNotSupported { command: "wait" });
            }
            if !cmdline.captures.is_empty() {
                bail!(CmdError::CapturesNotSupported { command: "wait" });
            }
            if !cmdline.redirects.is_empty() {
                bail!(CmdError::RedirectNotSupported { command: "wait" });
            }
            state.last_status = crate::task::try_wait(&cmdline.args, &mut state)?;
        }
        b"export" => {
            if cmdline.builtin {
                bail!(CmdError::BuiltinKeywordNotSupported { command: "export" });
            }
            if !cmdline.captures.is_empty() {
                bail!(CmdError::CapturesNotSupported { command: "export" });
            }
            if !cmdline.redirects.is_empty() {
                bail!(CmdError::RedirectNotSupported { command: "export" });
            }
            crate::exports::handle_export(&cmdline.args, &mut state)?;
            state.last_status = WaitStatus::Exited(0);
        }
        _ => return Ok(false),
    }
    Ok(true)
}
