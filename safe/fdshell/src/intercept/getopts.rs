// mutants: -C --lib -- -E 'test(/^intercept::getopts::/)'

//! `getopts <opts> name` — the plumbing is task #589 (the dispatch arm, help and
//! the `OPTIND` store), the parsing core is task #590 (optstring, word list,
//! per-word scan, and committing the counter and the intra-word cursor). The
//! caller's variables, the diagnostics and the statuses are task #591 and live in
//! the `vars` and `diagnostic` submodules; the driver here validates, resolves the
//! words, runs the scan and hands the result to them (bash 5.3.9, measured).

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

/// The scan entry: validate (the house rule for in-process commands), check the
/// operands, resolve the words, run the pure scan and commit the call (the
/// counters, `OPTARG`, the name, the statuses, the diagnostics).
pub(crate) fn run_getopts(
    line: &[u8],
    cmdline: &crate::parse::CommandLine,
    cell: &ForkCell<ShellState>,
) -> Result<bool, Report<CmdError>> {
    super::validation::validate_intercept_no_builtin(line, "getopts", cmdline)?;
    let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
    let (Some(optstring), Some(name)) = (cmdline.args.first(), cmdline.args.get(1)) else {
        // A missing operand is a fact about the line the user typed, so it is a
        // **status**, not a `Report`: bash prints its usage line and keeps running
        // (measured with `let`; a `bail!` here would abort the script), and no scan
        // runs — nothing is written (plan §3.1).
        diagnostic::print(&diagnostic::usage());
        state.set_last_exit(vars::USAGE_STATUS);
        return Ok(true);
    };
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
    commit(&mut state, &result, name, &parsed)?;
    Ok(true)
}

/// Commit one call in bash's measured order (plan §3.1): the counters and the
/// cursor, then `OPTARG`, then the **name** and its status, the diagnostic printed at
/// its own moment. Each arm is one row of the matrix of plan §2, every value measured
/// on bash 5.3.9: a found option names its own character and hands `OPTARG` the
/// argument it took (or unbinds the name); a presentation error is the `diagnostic`
/// row; the end of options answers `?`, an unbound `OPTARG` and status 1.
fn commit(
    state: &mut ShellState,
    result: &scan::Scan,
    name: &ShortCStr,
    parsed: &optstring::Optstring,
) -> Result<(), Report<CmdError>> {
    match result {
        scan::Scan::Found {
            ch,
            arg,
            next_index,
            next_char,
        } => {
            state.write_counter(*next_index)?;
            state.optchar = *next_char;
            state.write_optarg(arg.clone());
            vars::report_name(state, name, *ch, vars::REPORT_STATUS)
        }
        scan::Scan::Presentation { .. } => {
            diagnostic::presentation_row(state, name, result, parsed)
        }
        scan::Scan::End { index } => {
            state.write_counter(*index)?;
            state.write_optarg(None);
            vars::report_name(state, name, vars::QUESTION, vars::FAILURE_STATUS)
        }
    }
}

mod diagnostic;
mod optstring;
mod scan;
mod vars;
mod words;

#[cfg(test)]
mod tests;
