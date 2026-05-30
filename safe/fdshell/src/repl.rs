use crate::vars::FdVars;
use sys::siginfo::WaitStatus;

fn run(line: &str, fdvars: &mut FdVars, last_status: &mut WaitStatus) -> Result<i32, i32> {
    for part in line.split(';') {
        let part = part.trim();
        if !part.is_empty() {
            crate::run::run_one(part, fdvars, last_status)?;
        }
    }
    Ok(last_status.exit_code())
}

pub fn handle(line: &str, fdvars: &mut FdVars, last_status: &mut WaitStatus) -> Result<(), i32> {
    let code = run(line, fdvars, last_status)?;
    if code != 0 {
        eprintln!("exit code: {code}");
    }
    Ok(())
}

pub fn exec_cmd(line: &str, fdvars: &mut FdVars, last_status: &mut WaitStatus) -> Result<i32, i32> {
    run(line, fdvars, last_status)
}
