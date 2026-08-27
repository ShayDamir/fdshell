use crate::error::cmd::CmdError;
use crate::state::ShellState;
use error_stack::{Report, ResultExt};
use sys::ScriptText;
use sys::fork_cell::ForkCell;
use sys::{ImportedStr, ShortCStr, Trace};

/// Replace positional parameters with the args after `--`, set/clear a shell
/// option with `-o`/`+o`, or list variables (bare `set`) / fd variables
/// (`set -F`). Other `set` forms fall through to external lookup.
pub(crate) fn run_set(
    line: &[u8],
    cmdline: &crate::parse::CommandLine,
    text: &ScriptText,
    cell: &ForkCell<ShellState>,
) -> Result<bool, Report<CmdError>> {
    let Some(first) = cmdline.args.first() else {
        super::validation::validate_intercept(line, "set", cmdline)?;
        crate::xtrace::trace_cmd(b"set", cmdline, cell);
        return super::set_list::list_vars(cell).map(|_| true);
    };
    if first.eq_bytes(b"--") {
        crate::xtrace::trace_cmd(b"set", cmdline, cell);
        return run_set_positional(line, cmdline, text, cell);
    }
    if first.eq_bytes(b"-o") || first.eq_bytes(b"+o") {
        super::validation::validate_intercept(line, "set", cmdline)?;
        crate::xtrace::trace_cmd(b"set", cmdline, cell);
        return super::set_list::run_set_option(first, cmdline, cell).map(|_| true);
    }
    if first.eq_bytes(b"-x") || first.eq_bytes(b"+x") {
        super::validation::validate_intercept(line, "set", cmdline)?;
        return run_set_xtrace(first, cmdline, cell).map(|_| true);
    }
    if first.eq_bytes(b"-F") {
        super::validation::validate_intercept(line, "set", cmdline)?;
        crate::xtrace::trace_cmd(b"set", cmdline, cell);
        return super::set_list::list_fds(cell).map(|_| true);
    }
    if first.eq_bytes(b"--stdout-capture-limit") {
        super::validation::validate_intercept(line, "set", cmdline)?;
        crate::xtrace::trace_cmd(b"set", cmdline, cell);
        return super::set_limit::run_set_capture_limit(cmdline, cell).map(|_| true);
    }
    Ok(false)
}

fn run_set_positional(
    line: &[u8],
    cmdline: &crate::parse::CommandLine,
    text: &ScriptText,
    cell: &ForkCell<ShellState>,
) -> Result<bool, Report<CmdError>> {
    super::validation::validate_intercept(line, "set", cmdline)?;
    let expanded = crate::substitute::substitute_args(
        cmdline.args.get(1..).unwrap_or(&[]),
        cmdline.args_mask.get(1..).unwrap_or(&[]),
        cell,
    )
    .change_context(CmdError::Resolve)?;
    let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
    state.set_last_arg(expanded.last().cloned().unwrap_or_else(|| c"set".into()));
    let positional = expanded
        .iter()
        .map(|s| ImportedStr::new(s.clone(), Trace::at(text.start, text.origin.clone())))
        .collect();
    state.set_positional(positional);
    state.set_last_exit(0);
    Ok(true)
}

/// `set -x` enables, `set +x` disables xtrace. The command itself is printed
/// either way, as in bash.
fn run_set_xtrace(
    flag: &ShortCStr,
    cmdline: &crate::parse::CommandLine,
    cell: &ForkCell<ShellState>,
) -> Result<(), Report<CmdError>> {
    crate::xtrace::trace_unconditional(b"set", &cmdline.args);
    let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
    state.options =
        crate::options::set(state.options, crate::options::XTRACE, flag.eq_bytes(b"-x"));
    state.set_last_exit(0);
    Ok(())
}
