use crate::error::cmd::CmdError;
use crate::parse::CommandLine;
use crate::state::ShellState;
use core::fmt::Write;
use error_stack::Report;
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
    run_replace(line, cmdline, "exec", cell)
}

fn run_replace(
    line: &[u8],
    cmdline: &CommandLine,
    name: &'static str,
    cell: &ForkCell<ShellState>,
) -> Result<bool, Report<CmdError>> {
    super::validation::check_captures_not_supported(line, name, &cmdline.captures)?;

    let args = cmdline.args.clone();
    let args_fq = cmdline.args_fq.clone();
    let redirects = &cmdline.redirects;

    match crate::replacer::execute(&args, &args_fq, redirects, cell) {
        Ok(code) => sys::exit(code),
        Err(report) => {
            let _ = writeln!(crate::io::Stderr, "{report:?}");
            sys::exit(report.current_context().exit_code());
        }
    }
}
