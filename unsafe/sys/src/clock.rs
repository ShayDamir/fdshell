// mutants: -C --test -C clock -- -E 'binary(clock)'

use crate::SyscallError;

/// `clock_gettime(CLOCK_MONOTONIC)` in microseconds.
///
/// The monotonic clock never runs backwards (it counts from an arbitrary
/// boot-time origin), so a difference of two reads is a duration that cannot
/// underflow.
pub fn monotonic_us() -> Result<u64, SyscallError> {
    // SAFETY: `libc::timespec` is an all-integer struct; zeroed memory is a
    // valid value for it.
    let mut ts: libc::timespec = unsafe { core::mem::zeroed() };
    // SAFETY: `CLOCK_MONOTONIC` is a legal clock id and `ts` is writable
    // memory of exactly the size the kernel writes.
    crate::cvt(unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut ts) } as isize)?;
    Ok(us(&ts))
}

/// `clock_gettime(CLOCK_REALTIME)` in microseconds since the Unix epoch — the
/// wall clock the prompt layer formats for `\d \t \T \@ \A` (task #618).
pub fn realtime_us() -> Result<u64, SyscallError> {
    // SAFETY: `libc::timespec` is an all-integer struct; zeroed memory is a
    // valid value for it.
    let mut ts: libc::timespec = unsafe { core::mem::zeroed() };
    // SAFETY: `CLOCK_REALTIME` is a legal clock id and `ts` is writable
    // memory of exactly the size the kernel writes.
    crate::cvt(unsafe { libc::clock_gettime(libc::CLOCK_REALTIME, &mut ts) } as isize)?;
    Ok(us(&ts))
}

/// A `timespec` as total microseconds (`pub` so the integration test pins the
/// conversion against raw `libc::timespec` values).
pub fn us(t: &libc::timespec) -> u64 {
    (t.tv_sec as u64) * 1_000_000 + (t.tv_nsec as u64) / 1_000
}
