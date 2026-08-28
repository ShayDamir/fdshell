use builtins::error::Suggestion;
use error_stack::{Report, ResultExt};

use crate::error::cmd::CmdError;
use crate::intercept::ulimit_cmd::parse::Value;
use crate::intercept::ulimit_cmd::resources::Resource;
use sys::SyscallError;
use sys::rlimit::{self, RLimit};

/// Applies a limit change: scales by the resource's unit, merges with the
/// current limits per the H/S matrix (default scope: soft), and writes back.
pub(super) fn set_limit(
    res: Resource,
    hard: bool,
    soft: bool,
    value: Value,
) -> Result<(), Report<CmdError>> {
    let scaled = scale(res, value)?;
    let cur = rlimit::get(res.id).change_context(CmdError::UlimitGet)?;
    let next = RLimit {
        soft: if hard && !soft { cur.soft } else { scaled },
        hard: if soft && !hard { cur.hard } else { scaled },
    };
    match rlimit::set(res.id, next) {
        Ok(()) => Ok(()),
        // Raising a hard limit without privilege: a clean, actionable error.
        Err(SyscallError::EPERM(_)) => Err(Report::new(CmdError::UlimitSet).attach_opaque(
            Suggestion("raising a hard limit requires privilege; only lower it or run as root"),
        )),
        Err(e) => Err(Report::new(e).change_context(CmdError::UlimitSet)),
    }
}

/// The kernel value for a user value: `unlimited` passes through unscaled;
/// finite values are scaled by the resource's unit (overflow is a bad value).
pub(super) fn scale(res: Resource, value: Value) -> Result<u64, Report<CmdError>> {
    if value.amount == rlimit::UNLIMITED {
        return Ok(value.amount);
    }
    Ok(value
        .amount
        .checked_mul(res.scale())
        .ok_or(CmdError::UlimitBadValue { value: value.text })?)
}
