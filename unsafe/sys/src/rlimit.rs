use crate::SyscallError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RLimit {
    pub soft: u64,
    pub hard: u64,
}

/// Value meaning "no limit" (bash prints it as `unlimited`).
pub const UNLIMITED: u64 = libc::RLIM_INFINITY;

pub const CPU: u32 = libc::RLIMIT_CPU;
pub const DATA: u32 = libc::RLIMIT_DATA;
pub const FSIZE: u32 = libc::RLIMIT_FSIZE;
pub const CORE: u32 = libc::RLIMIT_CORE;
pub const MEMLOCK: u32 = libc::RLIMIT_MEMLOCK;
pub const RSS: u32 = libc::RLIMIT_RSS;
pub const NOFILE: u32 = libc::RLIMIT_NOFILE;
pub const STACK: u32 = libc::RLIMIT_STACK;
pub const NPROC: u32 = libc::RLIMIT_NPROC;
pub const AS: u32 = libc::RLIMIT_AS;

pub fn get(resource: u32) -> Result<RLimit, SyscallError> {
    // SAFETY: `rlim` is zero-initialized; `libc::rlimit` has only integer
    // fields, so zeroed memory is valid.
    let mut raw: libc::rlimit = unsafe { core::mem::zeroed() };
    // SAFETY: any `u32` is a legal `getrlimit` input; an unknown resource
    // returns `EINVAL`, caught by `cvt`.
    crate::cvt(unsafe { libc::getrlimit(resource, &mut raw) as isize })?;
    Ok(RLimit {
        soft: raw.rlim_cur,
        hard: raw.rlim_max,
    })
}

pub fn set(resource: u32, limit: RLimit) -> Result<(), SyscallError> {
    let raw = libc::rlimit {
        rlim_cur: limit.soft,
        rlim_max: limit.hard,
    };
    // SAFETY: `raw` holds plain integer values; an unknown resource returns
    // `EINVAL`, a privileged raise returns `EPERM` — both caught by `cvt`.
    crate::cvt(unsafe { libc::setrlimit(resource, &raw) as isize })?;
    Ok(())
}
