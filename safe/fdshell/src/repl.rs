use crate::task::Task;
use crate::vars::FdVars;
use std::collections::HashMap;
use sys::ShortCStr;
use sys::siginfo::WaitStatus;

fn run(
    line: &str,
    fdvars: &mut FdVars,
    tasks: &mut HashMap<ShortCStr, Task>,
    last_status: &mut WaitStatus,
) -> Result<i32, i32> {
    let mut start = 0;
    let mut in_quote = false;
    let bytes = line.as_bytes();
    let mut i = 0;
    while i <= bytes.len() {
        if bytes.get(i) == Some(&b'"') {
            in_quote = !in_quote;
        } else if i == bytes.len() || (!in_quote && bytes.get(i) == Some(&b';')) {
            let part = line[start..i].trim();
            if !part.is_empty() {
                run_cond_list(part, fdvars, tasks, last_status)?;
            }
            start = i + 1;
        }
        i += 1;
    }
    Ok(last_status.exit_code())
}

fn run_cond_list(
    line: &str,
    fdvars: &mut FdVars,
    tasks: &mut HashMap<ShortCStr, Task>,
    last_status: &mut WaitStatus,
) -> Result<(), i32> {
    let mut start = 0;
    let mut in_quote = false;
    let mut i = 0;
    while i <= line.len() {
        if line.as_bytes().get(i) == Some(&b'"') {
            in_quote = !in_quote;
        } else if i == line.len()
            || (!in_quote && (line[i..].starts_with("&&") || line[i..].starts_with("||")))
        {
            let part = line[start..i].trim();
            if !part.is_empty() {
                crate::run::run_one(part, fdvars, tasks, last_status)?;
                if i < line.len() {
                    let and = line[i..].starts_with("&&");
                    if (and && last_status.exit_code() != 0)
                        || (!and && last_status.exit_code() == 0)
                    {
                        return Ok(());
                    }
                }
            }
            start = i + 2;
            i += 1;
        }
        i += 1;
    }
    Ok(())
}

pub fn handle(
    line: &str,
    fdvars: &mut FdVars,
    tasks: &mut HashMap<ShortCStr, Task>,
    last_status: &mut WaitStatus,
) -> Result<(), i32> {
    let code = run(line, fdvars, tasks, last_status)?;
    if code != 0 {
        eprintln!("exit code: {code}");
    }
    Ok(())
}

pub fn exec_cmd(
    line: &str,
    fdvars: &mut FdVars,
    tasks: &mut HashMap<ShortCStr, Task>,
    last_status: &mut WaitStatus,
) -> Result<i32, i32> {
    run(line, fdvars, tasks, last_status)
}
