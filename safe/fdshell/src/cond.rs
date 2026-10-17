mod boundary;
mod errexit;
pub(crate) mod part;
mod parts;

use crate::error::cmd::CmdError;
use crate::loop_control::LoopControl;
use crate::scan::ScanState;
use crate::state::ShellState;
use alloc::vec::Vec;
use error_stack::{Report, ResultExt};
use sys::ScriptText;
use sys::fork_cell::ForkCell;

/// `exempt` disables errexit on the list's final segment (the conditions of
/// `if`/`while`/`until` are exempt, as in bash).
pub(crate) fn run_cond_list(
    text: &ScriptText,
    bodies: &[Vec<u8>],
    cell: &ForkCell<ShellState>,
    exempt: bool,
) -> Result<Option<LoopControl>, Report<CmdError>> {
    let line = text.as_bytes().change_context(CmdError::Never)?;
    let mut state = ScanState::new();
    let mut start = 0;
    let mut body_idx = 0;
    loop {
        let (i, is_or) = boundary::next_boundary(line, start, &mut state);
        let part_bodies = parts::take_bodies(line, start, i, bodies, &mut body_idx);
        if i == line.len() {
            if let Some(control) = part::run_part(text, line, start, i, &part_bodies, cell)? {
                return Ok(Some(control));
            }
            // Errexit only for a final part that actually ran: a list that
            // ended on a skipped `&&` chain keeps the failure in the exempt
            // preceding part and must not stop the shell.
            if !exempt && !parts::is_empty_part(line, start, i) && errexit::should_exit(cell)? {
                return Ok(Some(LoopControl::Exit));
            }
            break;
        }
        if !parts::is_empty_part(line, start, i) {
            if let Some(control) = part::run_part(text, line, start, i, &part_bodies, cell)? {
                return Ok(Some(control));
            }
            let st = cell.borrow().change_context(CmdError::Never)?;
            let ok = st.last_status.exit_code() == 0;
            if is_or && ok {
                // A short-circuited `||` chain gates bash's *AND-list*, and a
                // `;` ends it (task #236): skip the gated list, consume its
                // bodies, and resume the scan ungated on the statements
                // behind the separator — `echo a || if :; then :; fi; break`
                // skips the `if` and runs `break`.
                let after = i + 2;
                let gate = boundary::gate::gate_end(line, after, &mut state);
                parts::take_bodies(line, after, gate, bodies, &mut body_idx);
                start = gate;
                continue;
            }
            if !is_or && !ok {
                // `&&` after a failure: skip the rest of the list up to `||`.
                start = parts::skip_to_or(line, i + 2, &mut state, bodies, &mut body_idx);
                continue;
            }
        }
        start = i + 2;
    }
    Ok(None)
}

#[cfg(test)]
mod tests;
