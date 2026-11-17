// mutants: -C --lib -C --test -C positional -- -E 'test(/^intercept::tests::/) | binary(positional)'
use crate::error::cmd::CmdError;
use crate::parse::CommandLine;
use crate::state::ShellState;
use error_stack::{Report, ResultExt};
use sys::fork_cell::ForkCell;

pub(crate) fn run_shift(
    _line: &[u8],
    cmdline: &CommandLine,
    cell: &ForkCell<ShellState>,
) -> Result<bool, Report<CmdError>> {
    let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
    let n = match cmdline.args.first() {
        None => 1,
        Some(arg) => arg
            .parse::<usize>()
            .change_context(CmdError::InvalidArgument { arg: "shift count" })?,
    };
    // bash/POSIX: an `n` beyond the list is not a shift — the parameters stay put.
    if n <= state.positional.len() {
        state.shift(n);
    }
    state.set_last_exit(0);
    Ok(true)
}
