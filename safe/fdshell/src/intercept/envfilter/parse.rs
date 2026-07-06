//! Argument parsing for the envfilter builtin.
//!
//! Provides `parse_args` which parses `--allow`, `--deny`, `--clear`,
//! `--list`, and `--help` flags from command-line arguments.

use builtins::error::Suggestion;
use error_stack::{Report, ResultExt};
use sys::ShortCStr;

use crate::error::cmd::CmdError;
use crate::intercept::envfilter_display::print_help;
use crate::intercept::validation;

use super::{helpers, values};

#[derive(Default)]
pub(super) struct ParsedArgs {
    pub(super) do_list: bool,
    pub(super) do_clear: bool,
    pub(super) allow_patterns: Vec<ShortCStr>,
    pub(super) deny_patterns: Vec<ShortCStr>,
}

pub(super) fn parse_args(args: &[ShortCStr], line: &[u8]) -> Result<ParsedArgs, Report<CmdError>> {
    let mut do_list = false;
    let mut do_clear = false;
    let mut allow_patterns = Vec::new();
    let mut deny_patterns = Vec::new();
    let mut i = 0;
    while let Some(arg) = args.get(i) {
        let arg_bytes = arg.as_bytes().change_context(CmdError::Exec)?;
        if arg_bytes == b"--help" || arg_bytes == b"-h" {
            print_help();
            return Ok(ParsedArgs::default());
        } else if arg_bytes == b"--allow" {
            i = helpers::extend_pattern(args, line, i, "--allow", &mut allow_patterns)?;
        } else if arg_bytes == b"--deny" {
            i = helpers::extend_pattern(args, line, i, "--deny", &mut deny_patterns)?;
        } else if arg_bytes == b"--clear" {
            do_clear = true;
            i += 1;
        } else if arg_bytes == b"--list" {
            do_list = true;
            i += 1;
        } else {
            let pos = values::find_arg_pos(line, args, i);
            return Err(
                validation::err_at(line, pos, CmdError::EnvfilterUnknownFlag).attach_opaque(
                    Suggestion("valid flags: --allow, --deny, --list, --clear, --help"),
                ),
            );
        }
    }
    Ok(ParsedArgs {
        do_list,
        do_clear,
        allow_patterns,
        deny_patterns,
    })
}
