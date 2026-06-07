use crate::state::ShellState;
use sys::errno::EINVAL;
use sys::siginfo::WaitStatus;

pub(crate) fn run_one(line: &[u8], state: &mut ShellState) -> Result<(), i32> {
    match crate::parse::parse(line)? {
        crate::parse::ParsedLine::Cmd(cmdline) => {
            if crate::intercept::try_intercept(&cmdline, state)? {
                return Ok(());
            }
            let outcome = crate::launch::launch(state, &cmdline)?;
            state.last_status = crate::postlaunch::finish_cmd(cmdline, outcome, state)?;
        }
        crate::parse::ParsedLine::Pipeline(pipeline) => {
            state.last_status = crate::postlaunch::run_pipeline(pipeline, state)?;
        }
        crate::parse::ParsedLine::AssignFd { var, value } => {
            let src = state.fds.get(&value).ok_or(EINVAL)?;
            state.fds.insert(var, src.try_clone()?);
            state.last_status = WaitStatus::Exited(0);
        }
        crate::parse::ParsedLine::AssignStr { var, value } => {
            state.strings.insert(var, value);
            state.last_status = WaitStatus::Exited(0);
        }
        crate::parse::ParsedLine::Unset(var) => {
            state.fds.remove(&var);
            state.tasks.remove(&var);
            state.last_status = WaitStatus::Exited(0);
        }
        crate::parse::ParsedLine::Umask(mask) => {
            if let Some(m) = mask {
                sys::umask::set(m);
            } else {
                println!("{:04o}", sys::umask::get());
            }
            state.last_status = WaitStatus::Exited(0);
        }
        crate::parse::ParsedLine::For(_) => todo!(),
        crate::parse::ParsedLine::If(ifblock) => {
            let cond = ifblock.condition.as_bytes()?;
            crate::repl::run_cond_list(cond, state)?;
            if state.last_status.exit_code() == 0 {
                let then = ifblock.then_body.as_bytes()?;
                crate::repl::run_script(then, state)?;
            } else {
                let mut done = false;
                for (elif_cond, elif_body) in &ifblock.elifs {
                    let ec = elif_cond.as_bytes()?;
                    crate::repl::run_cond_list(ec, state)?;
                    if state.last_status.exit_code() == 0 {
                        let eb = elif_body.as_bytes()?;
                        crate::repl::run_script(eb, state)?;
                        done = true;
                        break;
                    }
                }
                if !done {
                    if let Some(ref else_body) = ifblock.else_body {
                        let eb = else_body.as_bytes()?;
                        crate::repl::run_script(eb, state)?;
                    } else {
                        state.last_status = WaitStatus::Exited(0);
                    }
                }
            }
        }
    }
    Ok(())
}
