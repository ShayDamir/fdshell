// mutants: -C --lib -C --test -C ulimit -- -E 'test(/^intercept::ulimit_cmd::/) | binary(ulimit)'

use builtins::error::Suggestion;
use error_stack::{Report, ResultExt};

use crate::error::cmd::CmdError;
use crate::intercept::ulimit_cmd::parse::{Keyword, Value};
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
    let cur = rlimit::get(res.id).change_context(CmdError::UlimitGet)?;
    let scaled = resolve(res, &value, cur)?;
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

/// A keyword value resolves to the current limit of the scope it names
/// (bash's `soft`/`hard` value words); it is already a kernel value, so the
/// measurement is indistinguishable from re-scaling — what is pinned is that
/// the value read back is the value written back.
fn resolve(res: Resource, value: &Value, cur: RLimit) -> Result<u64, Report<CmdError>> {
    match value.keyword {
        Some(Keyword::Soft) => Ok(cur.soft),
        Some(Keyword::Hard) => Ok(cur.hard),
        None => scale(res, value.clone()),
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
