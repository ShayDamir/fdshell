use crate::vars::FdVars;
use sys::errno::EINVAL;
use sys::siginfo::WaitStatus;

pub fn handle(line: &str, fdvars: &mut FdVars) -> Result<(), i32> {
    match crate::parse::parse(line)? {
        crate::parse::ParsedLine::Cmd(cmdline) => {
            if cmdline.command.as_bytes() == b"cd" {
                if cmdline.builtin || !cmdline.captures.is_empty() || !cmdline.redirects.is_empty()
                {
                    return Err(EINVAL);
                }
                return crate::cd::cd(&cmdline.args, fdvars);
            }
            let (status, capture_fd_opt) = crate::launch::launch(fdvars, &cmdline)?;
            match status {
                WaitStatus::Exited(0) => {
                    if let Some((capture_fd, child_pid)) = capture_fd_opt {
                        let entries = crate::capture::do_captures(
                            capture_fd,
                            child_pid,
                            cmdline.captures,
                            fdvars,
                        )?;
                        for (var, fd) in entries {
                            fdvars.insert(var, fd);
                        }
                    }
                }
                WaitStatus::Exited(n) => eprintln!("exit code: {n}"),
                _ => eprintln!("{status:?}"),
            }
        }
        crate::parse::ParsedLine::Pipeline(pipeline) => {
            let (status, channels) = crate::pipeline::launch_pipeline(fdvars, pipeline)?;
            match status {
                WaitStatus::Exited(0) => {
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
                WaitStatus::Exited(n) => eprintln!("exit code: {n}"),
                _ => eprintln!("{status:?}"),
            }
        }
        crate::parse::ParsedLine::Assign { var, value } => {
            let src = fdvars.resolve(value.as_bytes()).ok_or(EINVAL)?;
            fdvars.insert(var, src.try_clone()?);
        }
        crate::parse::ParsedLine::Unset(var) => {
            fdvars.remove(var.as_bytes());
        }
    }
    Ok(())
}
