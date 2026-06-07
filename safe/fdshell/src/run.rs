use crate::task::Task;
use crate::vars::FdVars;
use std::collections::HashMap;
use sys::ShortCStr;
use sys::errno::EINVAL;
use sys::siginfo::WaitStatus;

pub(crate) fn run_one(
    line: &[u8],
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
        crate::parse::ParsedLine::For(_) => todo!(),
        crate::parse::ParsedLine::If(ifblock) => {
            let mut cond_status = WaitStatus::Exited(0);
            let cond = ifblock.condition.as_bytes()?;
            crate::repl::run_cond_list(cond, fdvars, tasks, &mut cond_status)?;
            if cond_status.exit_code() == 0 {
                let then = ifblock.then_body.as_bytes()?;
                crate::repl::run_script(then, fdvars, tasks, last_status)?;
            } else {
                let mut done = false;
                for (elif_cond, elif_body) in &ifblock.elifs {
                    let ec = elif_cond.as_bytes()?;
                    crate::repl::run_cond_list(ec, fdvars, tasks, &mut cond_status)?;
                    if cond_status.exit_code() == 0 {
                        let eb = elif_body.as_bytes()?;
                        crate::repl::run_script(eb, fdvars, tasks, last_status)?;
                        done = true;
                        break;
                    }
                }
                if !done {
                    if let Some(ref else_body) = ifblock.else_body {
                        let eb = else_body.as_bytes()?;
                        crate::repl::run_script(eb, fdvars, tasks, last_status)?;
                    } else {
                        *last_status = WaitStatus::Exited(0);
                    }
                }
            }
        }
    }
    Ok(())
}
