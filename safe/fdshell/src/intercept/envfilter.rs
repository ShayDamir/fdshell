use builtins::error::Suggestion;
use error_stack::{Report, ResultExt};

use crate::error::cmd::CmdError;
use crate::intercept::validation::err_at;
use crate::state::ShellState;
use sys::fork_cell::ForkCell;

use super::envfilter_display::print_rules;

mod helpers;
mod parse;
mod values;

pub(crate) fn run_envfilter(
    line: &[u8],
    cmdline: &crate::parse::CommandLine,
    cell: &ForkCell<ShellState>,
) -> Result<bool, Report<CmdError>> {
    super::validation::validate_intercept(line, "envfilter", cmdline)?;

    if cmdline.args.is_empty() {
        return Err(
            err_at(line, 0, CmdError::EnvfilterNoArgs).attach_opaque(Suggestion(
                "valid flags: --allow, --deny, --list, --clear, --help",
            )),
        );
    }

    let parsed = parse::parse_args(&cmdline.args, line)?;

    if parsed.do_clear {
        let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
        state.env_filter.clear();
        return Ok(true);
    }

    if !parsed.allow_patterns.is_empty() || !parsed.deny_patterns.is_empty() {
        let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
        state.env_filter.allow.extend(parsed.allow_patterns);
        state.env_filter.deny.extend(parsed.deny_patterns);
    }

    if parsed.do_list {
        let state = cell.borrow().change_context(CmdError::Never)?;
        print_rules(&state.env_filter);
    }

    Ok(true)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests;
