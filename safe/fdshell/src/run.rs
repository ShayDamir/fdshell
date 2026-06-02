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
            if crate::intercept::try_intercept(&cmdline, fdvars, last_status)? {
                return Ok(());
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
            let src = fdvars.resolve(&value).ok_or(EINVAL)?;
            fdvars.insert(var, src.try_clone()?);
            *last_status = WaitStatus::Exited(0);
        }
        crate::parse::ParsedLine::Unset(var) => {
            fdvars.remove(&var);
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
