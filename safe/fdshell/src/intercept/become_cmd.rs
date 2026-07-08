use crate::parse::CommandLine;
use crate::state::ShellState;
use sys::fork_cell::ForkCell;

pub(crate) fn run_become(
    _line: &[u8],
    cmdline: &CommandLine,
    cell: &ForkCell<ShellState>,
) -> Result<bool, error_stack::Report<crate::error::cmd::CmdError>> {
    run_replace(cmdline, "become", cell)
}

pub(crate) fn run_exec(
    _line: &[u8],
    cmdline: &CommandLine,
    cell: &ForkCell<ShellState>,
) -> Result<bool, error_stack::Report<crate::error::cmd::CmdError>> {
    run_replace(cmdline, "exec", cell)
}

fn run_replace(
    cmdline: &CommandLine,
    name: &str,
    cell: &ForkCell<ShellState>,
) -> Result<bool, error_stack::Report<crate::error::cmd::CmdError>> {
    if !cmdline.captures.is_empty() {
        eprintln!("{name}: captures not supported");
        std::process::exit(sys::errno::EINVAL);
    }

    let args = cmdline.args.clone();
    let args_fq = cmdline.args_fq.clone();
    let redirects = &cmdline.redirects;

    match crate::replacer::execute(&args, &args_fq, redirects, cell) {
        Ok(code) => std::process::exit(code),
        Err(report) => {
            eprintln!("{:?}", report);
            std::process::exit(report.current_context().exit_code());
        }
    }
}
