use alloc::vec::Vec;
use error_stack::{Report, ResultExt};
use sys::ShortCStr;

use crate::error::cmd::CmdError;

/// Parsed `timeout` arguments: the deadline and the command to run.
#[cfg_attr(test, derive(Debug))]
pub struct TimeoutConfig {
    pub seconds: i64,
    pub command: ShortCStr,
    pub args: Vec<ShortCStr>,
    pub args_mask: Vec<Vec<bool>>,
}

/// Parses `timeout <seconds> <cmd> [args ...]`.
pub fn parse(
    args: &[ShortCStr],
    args_mask: &[Vec<bool>],
) -> Result<TimeoutConfig, Report<CmdError>> {
    let seconds_arg = args.first().ok_or(CmdError::TimeoutMissingSeconds)?;
    let seconds = parse_seconds(seconds_arg)?;
    let command = args.get(1).ok_or(CmdError::TimeoutMissingCommand)?;
    let sub_args: Vec<ShortCStr> = args.get(2..).unwrap_or_default().to_vec();
    let sub_args_mask: Vec<Vec<bool>> = args_mask.get(2..).unwrap_or_default().to_vec();
    Ok(TimeoutConfig {
        seconds,
        command: command.clone(),
        args: sub_args,
        args_mask: sub_args_mask,
    })
}

fn parse_seconds(s: &ShortCStr) -> Result<i64, Report<CmdError>> {
    let b = s.as_bytes().change_context(CmdError::Never)?;
    let n = core::str::from_utf8(b)
        .map_err(|_| Report::new(CmdError::TimeoutBadSeconds { value: s.clone() }))?;
    let v = n
        .parse::<i64>()
        .map_err(|_| Report::new(CmdError::TimeoutBadSeconds { value: s.clone() }))?;
    if v < 0 {
        return Err(Report::new(CmdError::TimeoutBadSeconds {
            value: s.clone(),
        }));
    }
    Ok(v)
}
