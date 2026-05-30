use crate::vars::FdVars;
use sys::errno::EINVAL;
use sys::siginfo::WaitStatus;

fn run(line: &str, fdvars: &mut FdVars) -> Result<i32, i32> {
    let mut last = 0;
    for part in line.split(';') {
        let part = part.trim();
        if !part.is_empty() {
            last = run_one(part, fdvars)?;
        }
    }
    Ok(last)
}

fn run_one(line: &str, fdvars: &mut FdVars) -> Result<i32, i32> {
    match crate::parse::parse(line)? {
        crate::parse::ParsedLine::Cmd(cmdline) => {
            if cmdline.command.as_bytes() == b"cd" {
                if cmdline.builtin || !cmdline.captures.is_empty() || !cmdline.redirects.is_empty()
                {
                    return Err(EINVAL);
                }
                crate::cd::cd(&cmdline.args, fdvars)?;
                return Ok(0);
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
            Ok(status.exit_code())
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
            Ok(status.exit_code())
        }
        crate::parse::ParsedLine::Assign { var, value } => {
            let src = fdvars.resolve(value.as_bytes()).ok_or(EINVAL)?;
            fdvars.insert(var, src.try_clone()?);
            Ok(0)
        }
        crate::parse::ParsedLine::Unset(var) => {
            fdvars.remove(var.as_bytes());
            Ok(0)
        }
    }
}

pub fn handle(line: &str, fdvars: &mut FdVars) -> Result<(), i32> {
    let code = run(line, fdvars)?;
    if code != 0 {
        eprintln!("exit code: {code}");
    }
    Ok(())
}

pub fn exec_cmd(line: &str, fdvars: &mut FdVars) -> Result<i32, i32> {
    run(line, fdvars)
}
