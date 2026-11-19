// mutants: -C --lib -C --test -C cond_blocks -C --test -C errexit -C --test -C set_opts -C --test -C group -C --test -C loops -- -E 'test(/^cond::/) | test(/^tests::run::/) | test(/^loop_::/) | test(/^if_exec::/) | binary(errexit) | binary(cond_blocks) | binary(set_opts) | binary(group) | binary(loops)'

pub(crate) mod list;
pub(crate) use list::run_list;

mod boundary;
pub(crate) mod errexit;
pub(crate) mod part;
mod parts;
mod units;

use crate::error::cmd::CmdError;
use crate::error::parse::ParseError;
use crate::loop_control::LoopControl;
use crate::state::ShellState;
use alloc::vec::Vec;
use error_stack::{Report, ResultExt};
use sys::ScriptText;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

/// The condition entry: the two parse-error pre-passes (bash parses the whole
/// input before running anything), then [`units`] — the shared scanner's split
/// at real separators (task #557), so a condition that lists several commands
/// runs **every** one and gates on the **last**, as bash's `compound_list`
/// does. Bash's errexit exemption is **one** mechanism: the execution-context
/// gate ([`errexit::gated`], [`crate::state::ShellState::errexit_gated`]). The
/// list's **non-final** parts run inside it, and the
/// `if`/`elif`/`while`/`until` **condition** callers (`if_exec.rs`, `loop_.rs`)
/// run a whole condition list inside it, because bash exempts the test *and*
/// everything a compound written in it executes — the flag riding the state is
/// what reaches through the nesting. A list reached ungated keeps exiting on a
/// failing final part; `should_exit` consults the flag.
/// `background` marks the list's **last** part to run asynchronously (the plain
/// `&` ends the statement after that part — task #326); the preceding parts of
/// the list run foreground.
pub(crate) fn run_cond_list(
    text: &ScriptText,
    bodies: &[Vec<u8>],
    cell: &ForkCell<ShellState>,
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
    // The same grammar sentence read from the other side: an operator that **opens**
    // a physical line has no command in front of it (task #406). The order above is
    // load-bearing — a text whose operator violates both is pinned on the right-side
    // message (`echo a || &&`), so the two rules must not trade that report.
    let opens_a_line = text.start.column == 1;
    if let Some(op) = parts::empty_left_side(line, opens_a_line) {
        return Err(
            Report::new(ParseError::ExpectedCommandBeforeOp { op }).change_context(CmdError::Parse)
        );
    }
    units::run_units(text, bodies, cell, background)
}

/// The condition's heredoc bodies as byte vectors (for [`run_cond_list`]),
/// built by the `if`/`while`/`until` callers (`if_exec.rs`, `loop_.rs`) that
/// own a condition's body list.
pub(crate) fn body_bytes(bodies: &[ShortCStr]) -> Vec<Vec<u8>> {
    bodies
        .iter()
        .filter_map(|b| b.as_bytes().ok().map(|bytes| bytes.to_vec()))
        .collect()
}

#[cfg(test)]
mod tests;
