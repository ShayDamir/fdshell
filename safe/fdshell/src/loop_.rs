use sys::ShortCStr;

pub(crate) fn run_loop(
    cond: &ShortCStr,
    body: &ShortCStr,
    invert: bool,
    state: &mut crate::state::ShellState,
) -> Result<(), i32> {
    loop {
        crate::repl::run_cond_list(cond.as_bytes()?, state)?;
        let exit_code = state.last_status.exit_code();
        if (exit_code == 0) != invert {
            break;
        }
        crate::repl::run_script(body.as_bytes()?, state)?;
    }
    Ok(())
}
