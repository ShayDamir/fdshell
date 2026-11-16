// mutants: -C --lib -C --test -C trap -- -E 'test(/^intercept::trap_cmd::/) | binary(trap)'

//! One condition operand: the `EXIT` slot, or a signal — by name through the
//! shared table (with the `SIG` prefix the list form prints, so the printed form
//! re-inputs), or by number. The range and the reserved pair are `trap`'s rules,
//! not the table's: `signalfd`'s semantics stay where they were.

use builtins::error::Suggestion;
use error_stack::{Report, ResultExt, bail};
use sys::ShortCStr;

use crate::error::cmd::CmdError;
use crate::intercept::signalfd_cmd::signals;
use crate::state::EXIT;

/// The condition that is not a signal, spelled by POSIX and by the list form alike.
const EXIT_NAME: &[u8] = b"EXIT";

/// The prefix the list form prints (bash's spelling, measured) and so the prefix
/// the parser must accept; the shared table keeps its names bare, for `signalfd`.
const SIG_PREFIX: &[u8] = b"SIG";

/// Resolve one condition operand to a condition number — the trap table's key.
/// Case is **not** folded: `usr1` is not a condition here. SIGKILL and SIGSTOP are
/// POSIX's undefined pair; judging them by number catches both spellings.
pub(crate) fn condition(field: &ShortCStr) -> Result<i32, Report<CmdError>> {
    let bytes = field.as_bytes().change_context(CmdError::Never)?;
    if bytes == EXIT_NAME {
        return Ok(EXIT);
    }
    let key = bytes.strip_prefix(SIG_PREFIX).unwrap_or(bytes);
    let Some(sig) = signals::resolve(key).or_else(|| kill_by_name(key)) else {
        return Err(bad_condition(field));
    };
    if sig == EXIT {
        return Ok(EXIT);
    }
    if !(1..=64).contains(&sig) {
        bail!(CmdError::TrapBadCondition {
            value: field.clone()
        });
    }
    if sig == sys::signal::SIGKILL || sig == sys::signalfd::SIGSTOP {
        return Err(reserved(field));
    }
    Ok(sig)
}

/// `KILL` is the one condition name the shared table has no row for — `signalfd`
/// cannot watch it — and POSIX still names it for `trap`, so the trap layer
/// resolves this single name itself, to judge it by number like `STOP`.
fn kill_by_name(key: &[u8]) -> Option<i32> {
    (key == b"KILL").then_some(sys::signal::SIGKILL)
}

/// bash's wording for an operand that is neither a name in the table nor a number
/// it can use.
fn bad_condition(field: &ShortCStr) -> Report<CmdError> {
    let report = Report::new(CmdError::TrapBadCondition {
        value: field.clone(),
    });
    report.attach_opaque(Suggestion(
        "a condition is EXIT, 0, or a signal name or number, without the SIG prefix",
    ))
}

/// SIGKILL and SIGSTOP: POSIX calls trapping them an undefined result, and
/// fdshell's answer is a clean rejection — a handler that can never run is worse
/// than an error the user can act on.
fn reserved(field: &ShortCStr) -> Report<CmdError> {
    let report = Report::new(CmdError::TrapReservedSignal {
        value: field.clone(),
    });
    report.attach_opaque(Suggestion(
        "SIGKILL and SIGSTOP cannot be caught or ignored; pick a catchable signal",
    ))
}
