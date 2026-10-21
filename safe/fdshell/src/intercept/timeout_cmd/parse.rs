use alloc::vec::Vec;
use error_stack::{Report, ResultExt, bail};
use sys::ShortCStr;

use crate::error::cmd::CmdError;
use crate::mask::MaskBit;

/// Parsed `timeout` arguments: the deadline and the command to run.
#[cfg_attr(test, derive(Debug))]
pub struct TimeoutConfig {
    pub seconds: i64,
    pub command: ShortCStr,
    pub command_mask: Vec<MaskBit>,
    pub command_span: Vec<usize>,
    pub command_quoted: bool,
    pub args: Vec<ShortCStr>,
    pub args_mask: Vec<Vec<MaskBit>>,
    pub args_span: Vec<Vec<usize>>,
    pub args_quoted: Vec<bool>,
}

/// Parses `timeout <seconds> <cmd> [args ...]`.
pub fn parse(
    args: &[ShortCStr],
    args_mask: &[Vec<MaskBit>],
    args_span: &[Vec<usize>],
    args_quoted: &[bool],
) -> Result<TimeoutConfig, Report<CmdError>> {
    let seconds_arg = args.first().ok_or(CmdError::TimeoutMissingSeconds)?;
    let seconds = parse_seconds(seconds_arg)?;
    // The target word is word 0 of the sub-command, so it stays raw here:
    // `command_word::expand` substitutes, folds, splits and globs it before the
    // launch (task #268) — folding it here would decode the single-quote
    // encoding before the substitution pass ever sees the word.
    let command = args.get(1).ok_or(CmdError::TimeoutMissingCommand)?.clone();
    let command_mask = args_mask.get(1).cloned().unwrap_or_default();
    let command_span = args_span.get(1).cloned().unwrap_or_default();
    let command_quoted = args_quoted.get(1).copied().unwrap_or(false);
    let sub_args: Vec<ShortCStr> = args.get(2..).unwrap_or_default().to_vec();
    let sub_args_mask: Vec<Vec<MaskBit>> = args_mask.get(2..).unwrap_or_default().to_vec();
    let sub_args_span: Vec<Vec<usize>> = args_span.get(2..).unwrap_or_default().to_vec();
    let sub_args_quoted: Vec<bool> = args_quoted.get(2..).unwrap_or_default().to_vec();
    Ok(TimeoutConfig {
        seconds,
        command,
        command_mask,
        command_span,
        command_quoted,
        args: sub_args,
        args_mask: sub_args_mask,
        args_span: sub_args_span,
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
