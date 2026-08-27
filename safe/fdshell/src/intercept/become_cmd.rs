use crate::error::cmd::CmdError;
use crate::parse::CommandLine;
use crate::state::ShellState;
use core::fmt::Write;
use error_stack::{Report, ResultExt};
use sys::fork_cell::ForkCell;

pub(crate) fn run_become(
    line: &[u8],
    cmdline: &CommandLine,
    cell: &ForkCell<ShellState>,
) -> Result<bool, Report<CmdError>> {
    run_replace(line, cmdline, "become", cell)
}

pub(crate) fn run_exec(
    line: &[u8],
    cmdline: &CommandLine,
    cell: &ForkCell<ShellState>,
) -> Result<bool, Report<CmdError>> {
    if cmdline.args.is_empty() && !cmdline.redirects.is_empty() {
        super::validation::check_captures_not_supported(line, "exec", &cmdline.captures)?;
        apply_redirects(cmdline, cell)?;
        let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
        state.set_last_exit(0);
        return Ok(true);
    }
    run_replace(line, cmdline, "exec", cell)
}

/// `exec` with redirections and no command: apply them to the shell's own fds.
fn apply_redirects(
    cmdline: &CommandLine,
    cell: &ForkCell<ShellState>,
) -> Result<(), Report<CmdError>> {
    let opened = crate::redirect::open_redirect_files(&cmdline.redirects, cell)
        .change_context(CmdError::Redirect)?;
    let resolved = crate::redirect::resolve_redirects(&cmdline.redirects, &opened, cell)
        .change_context(CmdError::Redirect)?;
    for r in &resolved {
        r.export().change_context(CmdError::Redirect)?;
    }
    // The cloned local fds in `resolved`/`opened` close on drop; the dup2'd
    // targets stay open in the shell.
    Ok(())
}

fn run_replace(
    line: &[u8],
    cmdline: &CommandLine,
    name: &'static str,
    cell: &ForkCell<ShellState>,
) -> Result<bool, Report<CmdError>> {
    super::validation::check_captures_not_supported(line, name, &cmdline.captures)?;

    let args = cmdline.args.clone();
    let args_mask = cmdline.args_mask.clone();
    let redirects = &cmdline.redirects;

    match crate::replacer::execute(&args, &args_mask, redirects, cell) {
        Ok(code) => sys::exit(code),
        Err(report) => {
            let _ = writeln!(crate::io::Stderr, "{report:?}");
            sys::exit(report.current_context().exit_code());
        }
    }
}

#[cfg(test)]
mod tests;
