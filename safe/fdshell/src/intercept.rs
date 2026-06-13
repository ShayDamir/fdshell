use crate::state::ShellState;
use sys::errno::EINVAL;
use sys::siginfo::WaitStatus;

pub(crate) fn try_intercept(
    cmdline: &crate::parse::CommandLine,
    state: &mut ShellState,
) -> Result<bool, i32> {
    match cmdline.command.as_bytes()? {
        b"cd" => {
            if cmdline.builtin || !cmdline.captures.is_empty() || !cmdline.redirects.is_empty() {
                return Err(EINVAL);
            }
            crate::cd::cd(&cmdline.args, state)?;
            state.last_status = WaitStatus::Exited(0);
        }
        b"exit" | b"quit" => {
            if cmdline.builtin || !cmdline.captures.is_empty() || !cmdline.redirects.is_empty() {
                return Err(EINVAL);
            }
            let code = match cmdline.args.first() {
                Some(arg) => {
                    let s = core::str::from_utf8(arg.as_bytes()?).map_err(|_| EINVAL)?;
                    s.parse::<i32>().map_err(|_| EINVAL)?
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
            crate::replacer::replace_shell(&cmdline.args, &cmdline.redirects, state);
        }
        b"export_fd" if cmdline.builtin => {
            if !cmdline.captures.is_empty() || !cmdline.redirects.is_empty() {
                return Err(EINVAL);
            }
            state.last_status = match crate::child::fdpass::export_fd(&cmdline.args, state) {
                Ok(()) => WaitStatus::Exited(0),
                Err(e) => WaitStatus::Exited(e),
            };
        }
        b"wait" => {
            if cmdline.builtin || !cmdline.captures.is_empty() || !cmdline.redirects.is_empty() {
                return Err(EINVAL);
            }
            state.last_status = crate::task::try_wait(&cmdline.args, state)?;
        }
        b"export" => {
            if cmdline.builtin || !cmdline.captures.is_empty() || !cmdline.redirects.is_empty() {
                return Err(EINVAL);
            }
            crate::exports::handle_export(&cmdline.args, state)?;
            state.last_status = WaitStatus::Exited(0);
        }
        _ => return Ok(false),
    }
    Ok(true)
}
