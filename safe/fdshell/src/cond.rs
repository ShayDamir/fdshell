mod boundary;
mod errexit;
pub(crate) mod part;
mod parts;

use crate::error::cmd::CmdError;
use crate::error::parse::ParseError;
use crate::loop_control::LoopControl;
use crate::scan::ScanState;
use crate::state::ShellState;
use alloc::vec::Vec;
use error_stack::{Report, ResultExt};
use sys::ScriptText;
use sys::fork_cell::ForkCell;

/// `exempt` disables errexit on the list's final segment (the conditions of
/// `if`/`while`/`until` are exempt, as in bash). The list's **non-final** parts
/// carry bash's other errexit suppression, the `&&`/`||` gate
/// ([`errexit::gated`]) — two independent suppressions of one check.
/// `background` marks the list's **last** part to run asynchronously (the plain
/// `&` ends the statement after that part — task #326); the preceding parts of
/// the list run foreground.
pub(crate) fn run_cond_list(
    text: &ScriptText,
    bodies: &[Vec<u8>],
    cell: &ForkCell<ShellState>,
    exempt: bool,
    background: bool,
) -> Result<Option<LoopControl>, Report<CmdError>> {
    let line = text.as_bytes().change_context(CmdError::Never)?;
    // An operator with no command behind it is a parse error, and it reports
    // **before** any part of the list runs — the house pre-pass shape, like
    // `script.rs`'s `AsyncInvalid` check — because bash's grammar requires a
    // command after `&&`/`||` and stops before running the list (task #367).
    // The walk is the shared one (task #492), so a list whose command lives
    // behind a physical-line end arrives here complete and never reports.
    if let Some(op) = parts::dangling_operator(line) {
        // `bail!` cannot carry the `.change_context()` this conversion needs
        // (STYLE §4.14): construct the report explicitly.
        return Err(
            Report::new(ParseError::ExpectedCommandAfterOp { op }).change_context(CmdError::Parse)
        );
    }
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
            if !exempt && !parts::is_empty_part(line, start, i) && errexit::should_exit(cell)? {
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

#[cfg(test)]
mod tests;
