use alloc::vec::Vec;
use error_stack::Report;
use sys::ShortCStr;

use crate::error::cmd::CmdError;
use crate::intercept::validation;

use super::values::{collect_values, find_arg_pos};

pub(super) fn extend_pattern(
    args: &[ShortCStr],
    line: &[u8],
    flag_idx: usize,
    flag_name: &'static str,
    patterns: &mut Vec<ShortCStr>,
) -> Result<usize, Report<CmdError>> {
    let next_idx = flag_idx + 1;
    match args.get(next_idx) {
        None => {
            let pos = find_arg_pos(line, args, flag_idx);
            return Err(validation::err_at(
                line,
                pos,
                CmdError::PatternRequired(flag_name),
            ));
        }
        Some(next) => {
            if next.starts_with(b"--") {
                let pos = find_arg_pos(line, args, flag_idx);
                return Err(validation::err_at(
                    line,
                    pos,
                    CmdError::PatternRequired(flag_name),
                ));
            }
        }
    }
    let next_i = collect_values(args, next_idx, patterns);
    Ok(next_i)
}
