// mutants: -C --lib -- -E 'test(/^intercept::getopts::/)'

//! `getopts <opts> name` — the plumbing is task #589 (the dispatch arm, help and
//! the `OPTIND` store), the parsing core is task #590 (optstring, word list,
//! per-word scan, and committing the counter and the intra-word cursor). Writing
//! the caller's variables, the diagnostics and the error statuses lands in task
//! #591; the driver here resolves the words, runs the scan and sets the
//! end-of-options status (bash 5.3.9: 1).

use error_stack::{Report, ResultExt};

use crate::error::cmd::CmdError;
use crate::state::ShellState;
use sys::ShortCStr;
use sys::UintParse;
use sys::fork_cell::ForkCell;

/// The counter's default: `getopts` starts at argument **1**, and bash 5.3.9
/// measures the seed as a visible variable (`set` answers `OPTIND=1`) that is
/// never exported. One owner — [`ShellState`]’s field seed, `unset_var`’s reset,
/// and the string store’s startup row (task #589).
pub(crate) const DEFAULT_OPTIND: u64 = 1;

/// The counter the name `OPTIND` describes, given the value just assigned and the
/// counter before it. Only an **exact** decimal moves it; an unparseable value
/// keeps the counter — bash 5.3.9 measures `OPTIND=abc` as "start at 1" while
/// `$OPTIND` still prints `abc`. The arithmetic-evaluator forms (`0x10`, `010`,
/// `3x`, `-1`) go through bash's arithmetic and are task #590's decision.
pub(crate) fn counter(value: &ShortCStr, current: u64) -> u64 {
    match value.parse_base0_uint() {
        UintParse::Exact(v) => v,
        _ => current,
    }
}

/// The byte of `word` at `offset`, `None` past its end — the per-byte helper the
/// getopts modules share (no new `unsafe/sys` API, task #590 §5).
pub(crate) fn byte_at(word: &ShortCStr, offset: usize) -> Option<u8> {
    let byte = word.get(offset..offset + 1)?;
    byte.as_bytes().ok()?.first().copied()
}

/// The scan entry: validate (the house rule for in-process commands), resolve
/// the words, run the pure scan, commit the counter and the cursor, and set the
/// end-of-options status. Variable writes and diagnostics are task #591's: this
/// task's pins assert the committed counters and the status, not stderr.
pub(crate) fn run_getopts(
    line: &[u8],
    cmdline: &crate::parse::CommandLine,
    cell: &ForkCell<ShellState>,
) -> Result<bool, Report<CmdError>> {
    super::validation::validate_intercept_no_builtin(line, "getopts", cmdline)?;
    let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
    let optstring = cmdline.args.first().ok_or(Report::new(CmdError::Never))?;
    let operands = cmdline.args.get(2..).ok_or(Report::new(CmdError::Never))?;
    let words = words::Words::resolve(operands, &state.positional);
    let parsed = optstring::Optstring::parse(optstring);
    let index = words.normalize(state.optind);
    let result = match words.word(index) {
        Some(word) => {
            let next = words.word(index + 1);
            scan::scan(index, &word, state.optchar, &parsed, next.as_ref())
        }
        None => scan::Scan::End {
            index: words.end_index(),
        },
    };
    commit(&mut state, &result);
    Ok(true)
}

/// Commit the scan: Found/Presentation write the next position (counter and
/// cursor), End writes the counter and the end-of-options status; bash 5.3.9
/// measures status 0 for Found **and** the presentation errors, status 1 only at
/// the end (task #591 owns the error rules the statuses will follow).
fn commit(state: &mut ShellState, result: &scan::Scan) {
    match result {
        scan::Scan::Found {
            next_index,
            next_char,
            ..
        }
        | scan::Scan::Presentation {
            next_index,
            next_char,
            ..
        } => {
            state.optind = *next_index;
            state.optchar = *next_char;
            state.set_last_exit(0);
        }
        scan::Scan::End { index } => {
            state.optind = *index;
            state.set_last_exit(1);
        }
    }
}

mod optstring;
mod scan;
mod words;

#[cfg(test)]
mod tests;
