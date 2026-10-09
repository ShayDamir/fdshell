use alloc::vec::Vec;
use error_stack::{Report, ResultExt, bail};
use sys::ShortCStr;

use crate::bytes::fold::fold_word;
use crate::error::cmd::CmdError;

/// Parsed `timeout` arguments: the deadline and the command to run.
#[cfg_attr(test, derive(Debug))]
pub struct TimeoutConfig {
    pub seconds: i64,
    pub command: ShortCStr,
    pub command_mask: Vec<bool>,
    pub args: Vec<ShortCStr>,
    pub args_mask: Vec<Vec<bool>>,
    pub args_quoted: Vec<bool>,
}

/// Parses `timeout <seconds> <cmd> [args ...]`.
pub fn parse(
    args: &[ShortCStr],
    args_mask: &[Vec<bool>],
    args_quoted: &[bool],
) -> Result<TimeoutConfig, Report<CmdError>> {
    let seconds_arg = args.first().ok_or(CmdError::TimeoutMissingSeconds)?;
    let seconds = parse_seconds(seconds_arg)?;
    // The target word never goes through substitution, so its unquoted escape
    // pairs are folded here (POSIX #4.1): `timeout 1 e\cho hi` runs `echo`, and
    // `timeout 1 a\*` runs the `PATH` file named `a*` (bash folds word 0).
    let raw = args.get(1).ok_or(CmdError::TimeoutMissingCommand)?;
    let (command, command_mask) =
        fold_word(raw, args_mask.get(1).map(Vec::as_slice).unwrap_or(&[]))
            .change_context(CmdError::Never)?;
    let sub_args: Vec<ShortCStr> = args.get(2..).unwrap_or_default().to_vec();
    let sub_args_mask: Vec<Vec<bool>> = args_mask.get(2..).unwrap_or_default().to_vec();
    let sub_args_quoted: Vec<bool> = args_quoted.get(2..).unwrap_or_default().to_vec();
    Ok(TimeoutConfig {
        seconds,
        command,
        command_mask,
        args: sub_args,
        args_mask: sub_args_mask,
        args_quoted: sub_args_quoted,
    })
}

fn parse_seconds(s: &ShortCStr) -> Result<i64, Report<CmdError>> {
    let b = s.as_bytes().change_context(CmdError::Never)?;
    let n =
        core::str::from_utf8(b).change_context(CmdError::TimeoutBadSeconds { value: s.clone() })?;
    let v = n
        .parse::<i64>()
        .change_context(CmdError::TimeoutBadSeconds { value: s.clone() })?;
    if v < 0 {
        bail!(CmdError::TimeoutBadSeconds { value: s.clone() });
    }
    Ok(v)
}
