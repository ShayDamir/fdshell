use crate::error::cmd::CmdError;
use crate::state::ShellState;
use error_stack::{Report, ResultExt};
use sys::ScriptText;
use sys::fork_cell::ForkCell;
use sys::{ImportedStr, ShortCStr, Trace};

/// Replace positional parameters with the args after `--`, or set/clear a
/// shell option with `-o`/`+o`. Other `set` forms fall through to external lookup.
pub(crate) fn run_set(
    line: &[u8],
    cmdline: &crate::parse::CommandLine,
    text: &ScriptText,
    cell: &ForkCell<ShellState>,
) -> Result<bool, Report<CmdError>> {
    let Some(first) = cmdline.args.first() else {
        return Ok(false);
    };
    if first.eq_bytes(b"--") {
        return run_set_positional(line, cmdline, text, cell);
    }
    if first.eq_bytes(b"-o") || first.eq_bytes(b"+o") {
        super::validation::validate_intercept(line, "set", cmdline)?;
        return run_set_option(first, cmdline, cell).map(|_| true);
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
        cmdline.args_fq.get(1..).unwrap_or(&[]),
        cell,
    )
    .change_context(CmdError::Resolve)?;
    let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
    let positional = expanded
        .iter()
        .map(|s| ImportedStr::new(s.clone(), Trace::at(text.start, text.origin.clone())))
        .collect();
    state.set_positional(positional);
    state.set_last_exit(0);
    Ok(true)
}

/// `set -o name` enables, `set +o name` disables; bare `set -o` lists options.
fn run_set_option(
    flag: &ShortCStr,
    cmdline: &crate::parse::CommandLine,
    cell: &ForkCell<ShellState>,
) -> Result<(), Report<CmdError>> {
    let enable = flag.eq_bytes(b"-o");
    let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
    match cmdline.args.get(1) {
        None => {
            sys::OUT
                .write_all(&crate::options::list(state.options))
                .ok();
            state.set_last_exit(0);
        }
        Some(name) => {
            let bit = crate::options::lookup(name).ok_or(CmdError::ShellOptionUnknown {
                command: "set",
                name: name.clone(),
            })?;
            state.options = crate::options::set(state.options, bit, enable);
            state.set_last_exit(0);
        }
    }
    Ok(())
}
