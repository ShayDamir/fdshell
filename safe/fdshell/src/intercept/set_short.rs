use crate::error::cmd::CmdError;
use crate::state::ShellState;
use error_stack::{Report, ResultExt};
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

/// Map a 2-byte `-c`/`+c` short flag to `(option bit, enable)`: the four
/// POSIX flags `set` intercepts (`e`, `u`, `f`, `v`). Anything else —
/// including `-x` (its own arm) and combined flags like `-eu` — is `None`.
pub(super) fn short_flag(first: &ShortCStr) -> Option<(u32, bool)> {
    let bytes = first.as_bytes().ok()?;
    // Exactly 2 bytes: a lone `-`/`+` or a combined flag (`-eu`) is not ours.
    let (sign, flag) = (*bytes.first()?, *bytes.get(1)?);
    if bytes.len() != 2 {
        return None;
    }
    let enable = sign == b'-';
    if !enable && sign != b'+' {
        return None;
    }
    let bit = match flag {
        b'e' => crate::options::ERREXIT,
        b'u' => crate::options::NOUNSET,
        b'f' => crate::options::NOGLOB,
        b'v' => crate::options::VERBOSITY,
        _ => return None,
    };
    Some((bit, enable))
}

/// `set -c` / `set +c`: set/clear one short-flag option, status 0 (the
/// ordinary xtrace is printed by the caller, as for `set -o`).
pub(super) fn run_set_short(
    bit: u32,
    enable: bool,
    cell: &ForkCell<ShellState>,
) -> Result<(), Report<CmdError>> {
    let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
    state.options = crate::options::set(state.options, bit, enable);
    state.set_last_exit(0);
    Ok(())
}
