// mutants: -C --lib -- -E 'test(/^intercept::getopts::/)'

//! The per-call writes to the **caller's** variables (task #591) — the two things
//! that make `getopts` an in-shell intercept instead of a forked builtin: the name
//! operand receives the option character, `OPTIND` the index the scan committed,
//! `OPTARG` the option's argument or **nothing at all** (an unbound name, not an empty
//! one: bash 5.3.9 measures `set -- -a; getopts "ab:c" o` and then answers
//! `${OPTARG+set}` empty). Both names stay **unexported** — nothing here touches
//! `exports`/`environ`, so they never reach a child environment (the #589 pins) — and
//! both are read back through the ordinary variable path.
//!
//! The measured matrix of plan §2 lives with the writers that drive it: the driver's
//! `commit` for the found option and the end of options, [`diagnostic`] for the
//! presentation rows the mode decides. The statuses below are bash's measured values,
//! not POSIX's (plan §4.3), and no message here is a `Report`: bash continues after
//! every one of them (plan §4.4).

use error_stack::{Report, ResultExt};

use sys::ImportedStr;
use sys::ShortCStr;

use crate::error::cmd::CmdError;
use crate::state::ShellState;

use super::diagnostic;

/// A report costs nothing: a found option, and every presentation error too (bash
/// measures 0 for both, plan §2).
pub(crate) const REPORT_STATUS: i32 = 0;

/// The end of options, and the processing error reachable here: a name operand that is
/// not an identifier. (bash's readonly row — `declare -r ro; getopts a ro` — needs a
/// `readonly` builtin fdshell does not have, plan §4.3.)
pub(crate) const FAILURE_STATUS: i32 = 1;

/// The usage line's status: POSIX says "> 1", bash 5.3.9 measures 2.
pub(crate) const USAGE_STATUS: i32 = 2;

/// The character `name` receives for an unknown option, for a verbose
/// missing-argument row, and at the end of options (measured, plan §2).
pub(crate) const QUESTION: u8 = b'?';

/// …and the one row that names something else: the **silent** mode's
/// missing-option-argument row answers `:` (POSIX and bash agree here).
pub(crate) const PLACEHOLDER: u8 = b':';

/// The name write and the status, last in bash's order (plan §3.1). `status` is the
/// row's; a name operand that is not an identifier answers bash's status **1** whatever
/// the row was (measured on a found option, at the end of options, and on the empty
/// operand), prints its own message, and **writes no variable** — the counter and
/// `OPTARG` committed by the row above stay written, which is what fixes the order:
/// `set -- -bval; getopts "ab:c" 1abc` commits OPTIND 2 and OPTARG `val`, names
/// nothing, and answers 1.
pub(crate) fn report_name(
    state: &mut ShellState,
    name: &ShortCStr,
    ch: u8,
    status: i32,
) -> Result<(), Report<CmdError>> {
    if !is_valid_name(name) {
        diagnostic::print(&diagnostic::invalid_identifier(
            &state.program_name.value,
            name,
        ));
        state.set_last_exit(FAILURE_STATUS);
        return Ok(());
    }
    write_name(state, name, ch)?;
    state.set_last_exit(status);
    Ok(())
}

/// bash's identifier rule for the name operand (measured, plan §2):
/// `[A-Za-z_][A-Za-z0-9_]*`, applied to the operand **after** expansion — so
/// `getopts "ab:c" "v"` is fine and `getopts "ab:c" "$n"` with `n=1abc` is the error.
/// Accepted: `v`, `_v`, `v1`, `a_1`, `z9`, `Z_`. Rejected: `1abc`, `v.x`, `a-b`, `%x`,
/// `-x`, `n=1`, and the empty operand. The rule sits with the write, not with the usage
/// check, because a rejected name stops only the write.
pub(crate) fn is_valid_name(name: &ShortCStr) -> bool {
    let Some(first) = name.get_byte(0) else {
        return false;
    };
    if !matches!(first, b'a'..=b'z' | b'A'..=b'Z' | b'_') {
        return false;
    }
    let mut offset = 1;
    while let Some(byte) = name.get_byte(offset) {
        if !matches!(byte, b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_') {
            return false;
        }
        offset += 1;
    }
    true
}

/// Write the caller's `name`: the one byte this call reports, through the ordinary
/// `set_var` path — so a name operand that happens to be `OPTIND` keeps that name's
/// counter-sync (task #589).
fn write_name(state: &mut ShellState, name: &ShortCStr, ch: u8) -> Result<(), Report<CmdError>> {
    let value = one_byte(ch).ok_or(Report::new(CmdError::Never))?;
    state.set_var(name.clone(), ImportedStr::shell(value));
    Ok(())
}

/// Write `OPTARG`: the argument through `set_var`, otherwise **unbind** the name —
/// bash's unbound, not empty (plan §2).
pub(crate) fn write_optarg(state: &mut ShellState, value: Option<ShortCStr>) {
    let name: ShortCStr = c"OPTARG".into();
    match value {
        Some(value) => state.set_var(name.clone(), ImportedStr::shell(value)),
        None => state.unset_var(&name),
    }
}

/// Write the counter **and its visible store row**: bash's `$OPTIND` is the counter, so
/// `set` lists it and `$OPTIND` reads it (the startup seed of `state/ctor.rs`). This is
/// the one write that must **not** go through `set_var`: for the name `OPTIND` that
/// setter is a name-sync, and an assignment resets the `optchar` cursor to 0 (task
/// #590's "re-read the word from its start" rule). Routing the scan's commit through it
/// would reset a cluster whose counter has not moved yet (`set -- -abc val`: `a`
/// reports, the counter stays 1, the cursor moves to 2) and that same option would be
/// reported forever. `set_var`/`unset_var` keep the **reverse** direction — a user
/// assignment — working; `state/setters.rs` points back here.
pub(crate) fn write_counter(state: &mut ShellState, index: u64) -> Result<(), Report<CmdError>> {
    let row = sys::format!("{index}").change_context(CmdError::Never)?;
    state.optind = index;
    state
        .strings
        .insert(c"OPTIND".into(), ImportedStr::shell(row));
    Ok(())
}

/// One byte as a store value — `None` only for a NUL, which no caller here can hand in:
/// every byte is read out of a NUL-free `ShortCStr` (the offending character of a
/// presentation row) or is a literal.
pub(crate) fn one_byte(ch: u8) -> Option<ShortCStr> {
    let mut value = ShortCStr::new();
    value.push_byte(ch).ok()?;
    Some(value)
}

#[cfg(test)]
mod tests;
