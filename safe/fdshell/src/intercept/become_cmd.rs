use crate::state::ShellState;
use sys::fork_cell::ForkCell;

pub(crate) fn run_become(
    _line: &[u8],
    cmdline: &crate::parse::CommandLine,
    _cell: &ForkCell<ShellState>,
) -> Result<bool, error_stack::Report<crate::error::cmd::CmdError>> {
    if !cmdline.captures.is_empty() {
        eprintln!("become: captures not supported");
        std::process::exit(sys::errno::EINVAL);
    }

    let args = cmdline.args.clone();
    let args_fq = cmdline.args_fq.clone();
    let redirects = &cmdline.redirects;

    match crate::replacer::execute(&args, &args_fq, redirects, _cell) {
        Ok(code) => std::process::exit(code),
        Err(report) => {
            eprintln!("{:?}", report);
            std::process::exit(report.current_context().exit_code());
        }
    }
}
