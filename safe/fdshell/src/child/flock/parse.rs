//! `flock` argument parsing. The `%var` comes from `args` (original); the
//! flags come from `refs` (substituted, though they are literals).

use core::ffi::CStr;
use error_stack::{Report, bail};
use sys::ShortCStr;

use builtins::error::BuiltinError;

use crate::child::fdops::args::var_arg;

#[cfg_attr(test, derive(Debug))]
pub(crate) struct FlockConfig {
    pub(crate) var: ShortCStr,
    pub(crate) operation: i32,
}

pub(crate) fn flock_parse(
    refs: &[&CStr],
    args: &[ShortCStr],
) -> Result<FlockConfig, Report<BuiltinError>> {
    if builtins::argparse::wants_help(refs) {
        bail!(BuiltinError::Help);
    }
    let var = var_arg(args)?;
    let operation = parse_flags(refs)?;
    Ok(FlockConfig { var, operation })
}

fn parse_flags(refs: &[&CStr]) -> Result<i32, Report<BuiltinError>> {
    let mut shared = false;
    let mut unlock = false;
    let mut nowait = false;
    for arg in refs.get(1..).unwrap_or(&[]) {
        match arg.to_bytes() {
            b"--shared" => shared = true,
            b"--unlock" => unlock = true,
            b"--wait" => {} // blocking is the default; accept and ignore
            b"--nowait" => nowait = true,
            _ => bail!(BuiltinError::InvalidArgument("flag")),
        }
    }
    if unlock && (shared || nowait) {
        bail!(BuiltinError::InvalidArgument("flag"));
    }
    let base = if unlock {
        sys::fcntl::LOCK_UN
    } else if shared {
        sys::fcntl::LOCK_SH
    } else {
        sys::fcntl::LOCK_EX
    };
    let operation = if nowait {
        base + sys::fcntl::LOCK_NB
    } else {
        base
    };
    Ok(operation)
}
