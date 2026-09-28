//! `verity %fd [--digest HEX]` or `verity %fd --enable [--algo sha256|sha512]`
//! — fs-verity (Linux >= 6.13) on an fd.
//!
//! Query: print `enabled=yes algo=sha256 digest=<hex>` or `enabled=no`.
//! Check (`--digest HEX`): exit 0 iff the file is verity and its digest matches
//! `HEX` (case-insensitive), else 1 (fail-closed). Enable (`--enable`):
//! provision verity in-kernel; permanent for the file's life, needs an
//! `O_RDONLY` fd of an inode the caller can write.

mod emit;
mod hex;
mod parse;

use builtins::error::BuiltinError;
use error_stack::{Report, ResultExt};
use sys::{LocalFd, ShortCStr};

use crate::state::ShellState;

use super::Ctx;

/// Merkle-tree block size for `--enable` (matches the pinned e2e digests).
const DEFAULT_BLOCK_SIZE: u32 = 4096;

pub(super) fn handle_verity(ctx: &Ctx) -> Result<i32, Report<BuiltinError>> {
    let cfg = parse::verity_parse(ctx.refs, ctx.args)?;
    let fd = resolve(&cfg.var, ctx.state)?;
    if cfg.enable {
        enable(fd, cfg.algo)?;
        return Ok(0);
    }
    // `MEASURE`: `ENODATA` means "not a verity file" (a normal outcome), not a fault.
    let digest = match fd.measure_verity() {
        Ok(d) => Some(d),
        Err(e) if e.errno() == sys::errno::ENODATA => None,
        Err(e) => return Err(syscall_err(e)),
    };
    if let Some(expected) = cfg.expected {
        return Ok(check(digest.as_ref(), &expected));
    }
    let line = emit::line(digest.as_ref()).change_context(BuiltinError::Io)?;
    sys::OUT.write_str(&line).change_context(BuiltinError::Io)?;
    Ok(0)
}

fn enable(fd: &LocalFd, algo: u32) -> Result<(), Report<BuiltinError>> {
    fd.enable_verity(algo, DEFAULT_BLOCK_SIZE)
        .change_context(BuiltinError::Syscall)
}

/// `--digest` check: 0 iff the file is verity and its digest matches, else 1.
fn check(digest: Option<&sys::fsverity::FsverityDigest>, expected: &[u8]) -> i32 {
    match digest {
        Some(d) if d.digest == expected => 0,
        _ => 1,
    }
}

/// Wrap a `SyscallError` as the report's inner error so `handle_builtin_error`
/// recovers its errno via `downcast_ref`.
fn syscall_err(e: sys::SyscallError) -> Report<BuiltinError> {
    match Err::<(), _>(e).change_context(BuiltinError::Syscall) {
        Err(r) => r,
        Ok(()) => Report::new(BuiltinError::Never),
    }
}

fn resolve<'a>(
    var: &ShortCStr,
    state: &'a ShellState,
) -> Result<&'a LocalFd, Report<BuiltinError>> {
    let found = state.fds.get(var).ok_or(BuiltinError::FdVarNotFound)?;
    Ok(&found.fd)
}

#[cfg(test)]
mod tests;
