use crate::error::cmd::CmdError;
use crate::state::ShellState;
use error_stack::{Report, ResultExt, bail};
use sys::ScriptText;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

/// `shopt -s/-u name…`, `shopt -q name`, or bare `shopt` (list).
pub(crate) fn run_shopt(
    line: &[u8],
    cmdline: &crate::parse::CommandLine,
    _text: &ScriptText,
    cell: &ForkCell<ShellState>,
) -> Result<bool, Report<CmdError>> {
    super::validation::validate_intercept(line, "shopt", cmdline)?;
    let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
    match cmdline.args.first() {
        None => {
            sys::OUT
                .write_all(&crate::options::list(state.options))
                .ok();
            state.set_last_exit(0);
        }
        Some(flag) if flag.eq_bytes(b"-q") => {
            let Some(name) = cmdline.args.get(1) else {
                bail!(CmdError::InvalidArgument { arg: "-q" });
            };
            let bit = lookup_error(name, "shopt")?;
            let code = (state.options & bit == 0) as i32;
            state.set_last_exit(code);
        }
        Some(flag) if flag.eq_bytes(b"-s") || flag.eq_bytes(b"-u") => {
            let enable = flag.eq_bytes(b"-s");
            for name in cmdline.args.get(1..).unwrap_or(&[]) {
                let bit = lookup_error(name, "shopt")?;
                state.options = crate::options::set(state.options, bit, enable);
            }
            state.set_last_exit(0);
        }
        Some(flag) => bail!(CmdError::ShellOptionUnknown {
            command: "shopt",
            name: flag.clone(),
        }),
    }
    Ok(true)
}

fn lookup_error(name: &ShortCStr, command: &'static str) -> Result<u32, Report<CmdError>> {
    Ok(
        crate::options::lookup(name).ok_or(CmdError::ShellOptionUnknown {
            command,
            name: name.clone(),
        })?,
    )
}
