use crate::state::ShellState;

pub(crate) use crate::cond::run_cond_list;
pub(crate) use crate::script::run_script;

pub fn handle(line: &[u8], state: &mut ShellState) -> Result<(), i32> {
    let code = run_script(line, state)?;
    if code != 0 {
        eprintln!("exit code: {code}");
    }
    Ok(())
}

pub fn exec_cmd(line: &[u8], state: &mut ShellState) -> Result<i32, i32> {
    run_script(line, state)
}
