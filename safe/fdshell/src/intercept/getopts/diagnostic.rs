// mutants: -C --lib -- -E 'test(/^intercept::getopts::/)'

//! The messages `getopts` prints **itself** (task #591), and the row they belong to:
//! the presentation rows are the ones the **mode** decides, so the mode's rules —
//! which character the name operand gets, where the offending character goes, and
//! whether anything is printed — live here with them. bash prints each message and
//! keeps running, so none of them is a `Report` (plan §4.4): a report would abort the
//! script, and the status belongs to the call, not to the statement layer. The writer
//! is best-effort — with stderr on `/dev/full` bash answers the very same statuses
//! (measured) — the `hash_cmd`/`type_cmd`/`xtrace` shape.
//!
//! The prefix rule (plan §3.3/§4.2), applied uniformly here: where bash's printed
//! line carries the shell name, print `$0` **as given** (fdshell stores `$0` as
//! given); the `line N:` field bash prepends to its internal-error lines is
//! **dropped**, because fdshell's builtin diagnostics carry no script-line prefix
//! (compare `hash: x: not found` with bash's `bash: line 1: hash: x: not found`); and
//! a message bash prints with no shell prefix at all — the usage line — stays
//! unprefixed.
//!
//! Everything here is built from **bytes**, so an offending byte reaches the message
//! as that byte and not as lossy text (plan §3.3): a non-ASCII option character is
//! pinned verbatim.

use core::ffi::CStr;

use error_stack::Report;

use sys::ShortCStr;

use crate::error::cmd::CmdError;
use crate::state::ShellState;

use super::optstring::Optstring;
use super::scan::{Problem, Scan};
use super::vars;

/// Apply one presentation row (plan §2), keeping the status at 0 and letting the scan
/// continue — `scan.rs`'s one-line decision. `name` answers `?`, except the **silent**
/// mode's missing-argument row, which answers `:`; `OPTARG` takes the offending
/// character only in the silent mode, where nothing is printed, and stays unbound in
/// the verbose one, which prints its message (all four rows measured on bash 5.3.9).
pub(crate) fn presentation_row(
    state: &mut ShellState,
    name: &ShortCStr,
    result: &Scan,
    parsed: &Optstring,
) -> Result<(), Report<CmdError>> {
    let Scan::Presentation {
        ch,
        problem,
        next_index,
        next_char,
    } = result
    else {
        return Err(Report::new(CmdError::Never));
    };
    let silent = parsed.is_silent();
    state.write_counter(*next_index)?;
    state.optchar = *next_char;
    state.write_optarg(if silent { vars::one_byte(*ch) } else { None });
    let prog = state.program_name.value.clone();
    if let Some(message) = message(silent, problem, &prog, *ch) {
        print(&message);
    }
    let colon = silent && matches!(problem, Problem::MissingArgument);
    let ch = if colon {
        vars::PLACEHOLDER
    } else {
        vars::QUESTION
    };
    vars::report_name(state, name, ch, vars::REPORT_STATUS)
}

/// The message of a presentation row: the **verbose** mode prints one for each
/// problem, the silent mode (a leading `:` in the optstring) prints nothing and takes
/// the offending character into `OPTARG` instead (bash 5.3.9 and POSIX agree on both
/// rows). `None` is the plan's "bash prints nothing".
fn message(silent: bool, problem: &Problem, prog: &ShortCStr, ch: u8) -> Option<ShortCStr> {
    if silent {
        return None;
    }
    let label = match problem {
        Problem::Unknown => c"illegal option -- ",
        Problem::MissingArgument => c"option requires an argument -- ",
    };
    build(prog, label, ch)
}

/// `<prog>: <label><ch>\n`, assembled from static pieces plus the offending byte
/// verbatim (`ShortCStr::concat` is infallible, STYLE §6.5).
fn build(prog: &ShortCStr, label: &'static CStr, ch: u8) -> Option<ShortCStr> {
    let offender = vars::one_byte(ch)?;
    let separator = ShortCStr::from(c": ");
    let newline = ShortCStr::from(c"\n");
    Some(ShortCStr::concat(&[
        prog,
        &separator,
        &ShortCStr::from(label),
        &offender,
        &newline,
    ]))
}

/// `<prog>: getopts: \`<name>': not a valid identifier` — bash's line carries a
/// `line N:` field before `getopts:`, dropped here (this file's header).
pub(crate) fn invalid_identifier(prog: &ShortCStr, name: &ShortCStr) -> ShortCStr {
    ShortCStr::concat(&[
        prog,
        &ShortCStr::from(c": getopts: `"),
        name,
        &ShortCStr::from(c"': not a valid identifier\n"),
    ])
}

/// The usage line, verbatim as bash prints it: no shell prefix, no `line N:`.
pub(crate) fn usage() -> ShortCStr {
    ShortCStr::from(c"getopts: usage: getopts optstring name [arg ...]\n")
}

/// Write one message to stderr, best-effort.
pub(crate) fn print(message: &ShortCStr) {
    let _ = sys::ERR.write_all(message.as_bytes().unwrap_or(&[]));
}

#[cfg(test)]
mod tests;
