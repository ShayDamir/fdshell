// mutants: -C --lib -C --test -C trap -- -E 'test(/^intercept::trap_cmd::/) | binary(trap)'

//! The list form: one `trap -- '<action>' <condition>` line per **registered**
//! condition, in the order the store gives (`EXIT` first, then ascending), and
//! nothing for a condition the shell holds no entry for — the default has no
//! entry, so it prints no line (POSIX XCU 2.15, bash 5.3.9 measured).
//!
//! The output is meant to be re-input, so it uses the shared `'`-quoting rule
//! (`intercept/quoting.rs`) and the shared name table
//! ([`crate::intercept::signalfd_cmd::signals`]); the `--` marker exists so an
//! action starting with a dash still parses.

use alloc::string::ToString;
use alloc::vec::Vec;
use error_stack::{Report, ResultExt};

use crate::error::cmd::CmdError;
use crate::intercept::quoting;
use crate::intercept::signalfd_cmd::signals;
use crate::state::{EXIT, ShellState, TrapDisposition};

/// Print the table to standard output, in one write.
///
/// Stays free: `trap_cmd.rs`'s list form is its only caller, so the chore's
/// multi-file criterion is not met (LESSONS: "A predicate whose only data is
/// the domain type's state is a method").
pub(crate) fn print(state: &ShellState) -> Result<(), Report<CmdError>> {
    let mut out = Vec::new();
    for (cond, disposition) in state.trap_list() {
        push_line(cond, disposition, &mut out)?;
    }
    sys::OUT.write_all(&out).ok();
    Ok(())
}

/// One line: `trap -- '<action>' <condition>`. POSIX's null action is the store's
/// [`crate::state::TrapDisposition::Ignore`] and prints an empty pair of quotes;
/// [`crate::state::TrapDisposition::Default`] is an **absent** entry (the
/// one-representation rule of the store), so its arm prints nothing and is
/// defence, not a form.
pub(super) fn push_line(
    cond: i32,
    disposition: &TrapDisposition,
    out: &mut Vec<u8>,
) -> Result<(), Report<CmdError>> {
    let action = match disposition {
        TrapDisposition::Action(body) => body.as_bytes().change_context(CmdError::Never)?,
        TrapDisposition::Ignore => &[],
        TrapDisposition::Default => return Ok(()),
    };
    out.extend_from_slice(b"trap -- ");
    quoting::push_quoted(action, out);
    out.extend_from_slice(b" ");
    push_condition(cond, out);
    out.push(b'\n');
    Ok(())
}

/// The condition's spelling, in the shape the parser takes back in: `EXIT` for the
/// store's slot, the shared table's name with the `SIG` prefix it prints, and the
/// decimal number for a signal the table has no name for (bash prints `32` for the
/// glibc-internal numbers, measured).
fn push_condition(cond: i32, out: &mut Vec<u8>) {
    if cond == EXIT {
        out.extend_from_slice(b"EXIT");
        return;
    }
    match signals::signal_name(cond) {
        Some(bare) => {
            out.extend_from_slice(b"SIG");
            out.extend_from_slice(bare);
        }
        None => out.extend_from_slice(cond.to_string().as_bytes()),
    }
}
