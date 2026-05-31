use crate::vars::FdVars;
use sys::errno::EINVAL;
use sys::siginfo::WaitStatus;

pub(crate) fn run_one(
    line: &str,
    fdvars: &mut FdVars,
    last_status: &mut WaitStatus,
) -> Result<(), i32> {
    match crate::parse::parse(line)? {
        crate::parse::ParsedLine::Cmd(cmdline) => {
            if cmdline.command.as_bytes() == b"cd" {
                if cmdline.builtin || !cmdline.captures.is_empty() || !cmdline.redirects.is_empty()
                {
                    return Err(EINVAL);
                }
                crate::cd::cd(&cmdline.args, fdvars)?;
                *last_status = WaitStatus::Exited(0);
                return Ok(());
            }
            if cmdline.command.as_bytes() == b"exit" || cmdline.command.as_bytes() == b"quit" {
                if cmdline.builtin || !cmdline.captures.is_empty() || !cmdline.redirects.is_empty()
                {
                    return Err(EINVAL);
                }
                let code = match cmdline.args.first() {
                    Some(arg) => {
                        let s = core::str::from_utf8(arg.as_bytes()).map_err(|_| EINVAL)?;
                        s.parse::<i32>().map_err(|_| EINVAL)?
                    }
                    None => last_status.exit_code(),
                };
                std::process::exit(code);
            }
            if cmdline.command.as_bytes() == b"become" {
                if !cmdline.captures.is_empty() {
                    eprintln!("become: captures not supported");
                    std::process::exit(sys::errno::EINVAL);
                }
                crate::replacer::replace_shell(&cmdline.args, &cmdline.redirects, fdvars);
            }
            let (status, capture_fd_opt) = crate::launch::launch(fdvars, &cmdline)?;
            if let WaitStatus::Exited(0) = status
                && let Some((capture_fd, child_pid)) = capture_fd_opt
            {
                let entries =
                    crate::capture::do_captures(capture_fd, child_pid, cmdline.captures, fdvars)?;
                for (var, fd) in entries {
                    fdvars.insert(var, fd);
                }
            }
            *last_status = status;
        }
        crate::parse::ParsedLine::Pipeline(pipeline) => {
            let (status, channels) = crate::pipeline::launch_pipeline(fdvars, pipeline)?;
            if let WaitStatus::Exited(0) = status {
                for ch in channels {
                    let entries = crate::capture::do_captures(
                        ch.capture_fd,
                        ch.child_pid,
                        ch.captures,
                        fdvars,
                    )?;
                    for (var, fd) in entries {
                        fdvars.insert(var, fd);
                    }
                }
            }
            *last_status = status;
        }
        crate::parse::ParsedLine::Assign { var, value } => {
            let src = fdvars.resolve(value.as_bytes()).ok_or(EINVAL)?;
            fdvars.insert(var, src.try_clone()?);
            *last_status = WaitStatus::Exited(0);
        }
        crate::parse::ParsedLine::Unset(var) => {
            fdvars.remove(var.as_bytes());
            *last_status = WaitStatus::Exited(0);
        }
        crate::parse::ParsedLine::Umask(mask) => {
            match mask {
                Some(m) => {
                    sys::umask::set(m);
                }
                None => println!("{:04o}", sys::umask::get()),
            }
            *last_status = WaitStatus::Exited(0);
        }
    }
    Ok(())
}
