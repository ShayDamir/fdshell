use crate::vars::FdVars;
use sys::errno::EINVAL;
use sys::siginfo::WaitStatus;

pub(crate) fn try_intercept(
    cmdline: &crate::parse::CommandLine,
    fdvars: &mut FdVars,
    last_status: &mut WaitStatus,
) -> Result<bool, i32> {
    match cmdline.command.as_bytes()? {
        b"cd" => {
            if cmdline.builtin || !cmdline.captures.is_empty() || !cmdline.redirects.is_empty() {
                return Err(EINVAL);
            }
            crate::cd::cd(&cmdline.args, fdvars)?;
            *last_status = WaitStatus::Exited(0);
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
                None => last_status.exit_code(),
            };
            std::process::exit(code);
        }
        b"become" => {
            if !cmdline.captures.is_empty() {
                eprintln!("become: captures not supported");
                std::process::exit(sys::errno::EINVAL);
            }
            crate::replacer::replace_shell(&cmdline.args, &cmdline.redirects, fdvars);
        }
        b"export_fd" if cmdline.builtin => {
            if !cmdline.captures.is_empty() || !cmdline.redirects.is_empty() {
                return Err(EINVAL);
            }
            *last_status = match crate::child::fdpass::export_fd(&cmdline.args, fdvars) {
                Ok(()) => WaitStatus::Exited(0),
                Err(e) => WaitStatus::Exited(e),
            };
        }
        _ => return Ok(false),
    }
    Ok(true)
}
