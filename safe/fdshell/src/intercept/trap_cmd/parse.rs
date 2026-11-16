// mutants: -C --lib -C --test -C trap -- -E 'test(/^intercept::trap_cmd::/) | binary(trap)'

//! The `trap` grammar: which form the words spell. No state, no syscalls — the
//! store is `state/traps.rs`, the dispositions are `apply`'s, one condition operand
//! is `cond`'s; the split is what lets the grammar be tested alone.

use alloc::vec::Vec;
use builtins::error::Suggestion;
use error_stack::Report;
use sys::ShortCStr;

use crate::error::cmd::CmdError;
use crate::state::TrapDisposition;

/// What one call stores per condition, and the condition operands it applies it
/// to. The operands stay words: each is resolved one at a time, because an invalid
/// one is reported without undoing the ones before it.
#[cfg_attr(test, derive(Debug))]
pub(crate) struct Request {
    pub(crate) disposition: TrapDisposition,
    pub(crate) conditions: Vec<ShortCStr>,
}

/// The end-of-options marker, printed by the list form so its output re-inputs —
/// the printer and the parser are one contract, and this is the option they agree
/// on (`trap -- 'echo x' SIGUSR1` is a registration, not a marker plus a name).
const END_OF_OPTIONS: &[u8] = b"--";

/// The form the words spell; `None` is the list form (no action operand at all).
pub(crate) fn form(fields: &[ShortCStr]) -> Result<Option<Request>, Report<CmdError>> {
    let fields = after_end_of_options(fields);
    let Some(action) = fields.first() else {
        return Ok(None);
    };
    // POSIX XCU 2.15: a first operand that is a decimal integer makes **every**
    // operand a condition, and the action is the default — `trap 0` resets `EXIT`.
    if action.parse::<i32>().is_ok() {
        return Ok(Some(request(TrapDisposition::Default, fields)));
    }
    let operands = fields.get(1..).unwrap_or(&[]);
    if operands.is_empty() {
        return Err(usage());
    }
    let disposition = if action.eq_bytes(b"-") {
        TrapDisposition::Default
    } else if action.is_empty() {
        TrapDisposition::Ignore
    } else {
        TrapDisposition::Action(action.clone())
    };
    Ok(Some(request(disposition, operands)))
}

/// The words after the marker, if the marker is there.
fn after_end_of_options(fields: &[ShortCStr]) -> &[ShortCStr] {
    match fields.first() {
        Some(field) if field.eq_bytes(END_OF_OPTIONS) => fields.get(1..).unwrap_or(&[]),
        _ => fields,
    }
}

/// One request, from the disposition the form spells and the operands it named.
fn request(disposition: TrapDisposition, conditions: &[ShortCStr]) -> Request {
    Request {
        disposition,
        conditions: conditions.to_vec(),
    }
}

/// The usage report: an action with nothing to apply it to (POSIX's `-` and null
/// action both need at least one condition; bash answers 2).
fn usage() -> Report<CmdError> {
    let report = Report::new(CmdError::TrapUsage);
    report.attach_opaque(Suggestion(
        "give an action and at least one condition, or run `trap` alone to list",
    ))
}
