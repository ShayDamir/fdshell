use crate::task::Task;
use crate::vars::FdVars;
use std::collections::HashMap;
use sys::ShortCStr;
use sys::errno::EINVAL;
use sys::siginfo::WaitStatus;

pub(crate) fn run_one(
    line: &str,
    fdvars: &mut FdVars,
    tasks: &mut HashMap<ShortCStr, Task>,
    last_status: &mut WaitStatus,
) -> Result<(), i32> {
    match crate::parse::parse(line)? {
        crate::parse::ParsedLine::Cmd(cmdline) => {
            if crate::intercept::try_intercept(&cmdline, fdvars, tasks, last_status)? {
                return Ok(());
            }
            let outcome = crate::launch::launch(fdvars, &cmdline)?;
            *last_status = crate::postlaunch::finish_cmd(cmdline, outcome, fdvars, tasks)?;
        }
        crate::parse::ParsedLine::Pipeline(pipeline) => {
            *last_status = crate::postlaunch::run_pipeline(pipeline, fdvars)?;
        }
        crate::parse::ParsedLine::Assign { var, value } => {
            let src = fdvars.resolve(&value).ok_or(EINVAL)?;
            fdvars.insert(var, src.try_clone()?);
            *last_status = WaitStatus::Exited(0);
        }
        crate::parse::ParsedLine::Unset(var) => {
            fdvars.remove(&var);
            tasks.remove(&var);
            *last_status = WaitStatus::Exited(0);
        }
        crate::parse::ParsedLine::Umask(mask) => {
            if let Some(m) = mask {
                sys::umask::set(m);
            } else {
                println!("{:04o}", sys::umask::get());
            }
            *last_status = WaitStatus::Exited(0);
        }
    }
    Ok(())
}
