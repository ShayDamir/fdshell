// mutants: -C --lib -C --test -C positional -- -E 'test(/^intercept::tests::/) | binary(positional)'
use crate::error::cmd::CmdError;
use crate::numarg::{LEGAL_NUMBER, decimal};
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
        Some(arg) => {
            let bytes = arg.as_bytes().change_context(CmdError::Never)?;
            // bash's `legal_number` bounds at the intmax range, and a
            // negative count is the "shift count out of range" rejection.
            decimal(bytes, &LEGAL_NUMBER)
                .filter(|n| (0..=i64::MAX as i128).contains(n))
                .and_then(|n| usize::try_from(n).ok())
                .ok_or(CmdError::InvalidArgument { arg: "shift count" })?
        }
    };
    // bash/POSIX: an `n` beyond the list is not a shift — the parameters stay put.
    if n <= state.positional.len() {
        state.shift(n);
    }
    state.set_last_exit(0);
    Ok(true)
}
