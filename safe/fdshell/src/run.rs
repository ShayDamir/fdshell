use std::collections::HashMap;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

use crate::error::parse::{ParseErrorInfo, to_parse_err};
use crate::state::ShellState;
use sys::errno::EINVAL;
use sys::siginfo::WaitStatus;

pub(crate) fn run_one(line: &[u8], cell: &ForkCell<ShellState>) -> Result<(), ParseErrorInfo> {
    match crate::parse::parse(line)? {
        crate::parse::ParsedLine::Cmd(cmdline) => {
            if crate::intercept::try_intercept(&cmdline, cell).map_err(to_parse_err)? {
                return Ok(());
            }
            let outcome = crate::launch::launch(cell, &cmdline).map_err(to_parse_err)?;
            {
                let mut state = cell.borrow_mut().map_err(to_parse_err)?;
                state.last_status = crate::postlaunch::finish_cmd(cmdline, outcome, &mut state)
                    .map_err(to_parse_err)?;
            }
        }
        crate::parse::ParsedLine::Pipeline(pipeline) => {
            let status = crate::postlaunch::run_pipeline(pipeline, cell).map_err(to_parse_err)?;
            let mut state = cell.borrow_mut().map_err(to_parse_err)?;
            state.last_status = status;
        }
        crate::parse::ParsedLine::AssignFd { var, value } => {
            let mut state = cell.borrow_mut().map_err(to_parse_err)?;
            let src = state.fds.get(&value).ok_or(EINVAL)?.try_clone()?;
            state.fds.insert(var, src);
            state.last_status = WaitStatus::Exited(0);
        }
        crate::parse::ParsedLine::AssignStr { var, value } => {
            let expanded = crate::substitute::substitute_arg(&value, &mut HashMap::new(), cell)?;
            let mut state = cell.borrow_mut().map_err(to_parse_err)?;
            state
                .strings
                .insert(var, ShortCStr::from_vec(expanded.into_bytes())?);
            state.last_status = WaitStatus::Exited(0);
        }
        crate::parse::ParsedLine::Unset(var) => {
            let mut state = cell.borrow_mut().map_err(to_parse_err)?;
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
            let mut state = cell.borrow_mut().map_err(to_parse_err)?;
            state.last_status = WaitStatus::Exited(0);
        }
        crate::parse::ParsedLine::For(forblock) => {
            {
                let mut state = cell.borrow_mut().map_err(to_parse_err)?;
                state.last_status = WaitStatus::Exited(0);
            }
            let words = crate::expand::expand_for_words(&forblock.words, cell)?;
            for word in &words {
                {
                    let mut state = cell.borrow_mut().map_err(to_parse_err)?;
                    state.strings.insert(forblock.var.clone(), word.clone());
                }
                crate::repl::run_script(forblock.body.as_bytes()?, cell)?;
            }
        }
        crate::parse::ParsedLine::While(whileblock) => {
            crate::loop_::run_loop(&whileblock.condition, &whileblock.body, true, cell)?;
        }
        crate::parse::ParsedLine::Until(untilblock) => {
            crate::loop_::run_loop(&untilblock.condition, &untilblock.body, false, cell)?;
        }
        crate::parse::ParsedLine::If(ifblock) => {
            crate::if_exec::run_if(&ifblock, cell)?;
        }
    }
    Ok(())
}
