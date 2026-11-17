//! The operator walk of one unit, split out of `cond.rs` (task #576). The
//! entry (`run_cond_list`) keeps the parse-error pre-passes; this walk adds no
//! error paths of its own.

// mutants: -C --lib -C --test -C cond_blocks -C --test -C errexit -C --test -C set_opts -C --test -C group -C --test -C loops -- -E 'test(/^cond::/) | test(/^tests::run::/) | binary(errexit) | binary(cond_blocks) | binary(set_opts) | binary(group) | binary(loops)'

use super::{boundary, errexit, part, parts};
use crate::error::cmd::CmdError;
use crate::loop_control::LoopControl;
use crate::scan::ScanState;
use crate::state::ShellState;
use alloc::vec::Vec;
use error_stack::{Report, ResultExt};
use sys::ScriptText;
use sys::fork_cell::ForkCell;

/// The operator walk over one unit — the whole text when the unit scan splits
/// nothing (every pinned single-unit condition and top-level statement runs
/// through here byte-identical), or one split unit of a multi-unit condition
/// (task #557, [`super::units`]). The entry's pre-passes run once over the
/// whole text **before** anything runs, because bash parses the whole input
/// before running anything (tasks #367/#406).
pub(crate) fn run_list(
    text: &ScriptText,
    bodies: &[Vec<u8>],
    cell: &ForkCell<ShellState>,
    background: bool,
) -> Result<Option<LoopControl>, Report<CmdError>> {
    let line = text.as_bytes().change_context(CmdError::Never)?;
    let mut state = ScanState::new();
    let mut start = 0;
    let mut body_idx = 0;
    loop {
        let (i, is_or) = boundary::next_boundary(line, start, &mut state);
        let part_bodies = parts::take_bodies(line, start, i, bodies, &mut body_idx);
        if i == line.len() {
            if let Some(control) =
                part::run_part(text, line, start, i, &part_bodies, cell, background)?
            {
                return Ok(Some(control));
            }
            // Errexit only for a final part that actually ran: a list that
            // ended on a skipped `&&` chain keeps the failure in the exempt
            // preceding part and must not stop the shell. The other half of
            // bash's rule is the gate below: every **non-final** part runs
            // inside `errexit::gated`, so a compound sitting there (loop body,
            // group, `if` body, `case` arm, function frame) never consults
            // `should_exit`; the final part is the command following the final
            // operator, so it keeps exiting.
            if !parts::is_empty_part(line, start, i) && errexit::should_exit(cell)? {
                return Ok(Some(LoopControl::Exit));
            }
            break;
        }
        if !parts::is_empty_part(line, start, i) {
            // A non-final part of the list always runs foreground: the plain
            // `&` flag marks only the list's last part.
            if let Some(control) = errexit::gated(cell, || {
                part::run_part(text, line, start, i, &part_bodies, cell, false)
            })? {
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
