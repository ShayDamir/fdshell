#![allow(clippy::unwrap_used)]

use sys::getrusage;

/// Burn user-mode CPU the compiler cannot fold away: the accumulator is fed
/// through `black_box` on *every* iteration, so release builds execute the
/// loop too and the burned time is real (LESSONS.md: "A CPU-burn timing test
/// measures nothing in a release build").
fn burn_cpu(iterations: u64) -> u64 {
    let mut n = 0u64;

    while n < iterations {
        n = core::hint::black_box(n.wrapping_add(1));
    }

    core::hint::black_box(&n);
    n
}

/// `self_usage()` succeeds and reports a positive own-user time.
#[test]
fn self_usage_succeeds() {
    // Burn a little CPU so the reported user time is deterministically
    // non-zero (a freshly started process can read `utime: 0`, and an
    // optimizable burn reads 0 too: 50M un-foldable iterations cost tens of
    // ms of user time in release builds).
    burn_cpu(50_000_000);
    let t = getrusage::self_usage().unwrap();
    // u64 fields are finite by construction; the process has burned user time
    // by now, so `utime` must be positive.
    assert!(t.utime > 0, "utime must be positive: {t:?}");
}

/// `CpuTimes::from_rusage` converts the `timeval` fields to total
/// microseconds.
#[test]
fn from_rusage_converts_timeval_to_microseconds() {
    // SAFETY: `libc::rusage` is all-integer; zeroed memory is valid.
    let mut raw: libc::rusage = unsafe { core::mem::zeroed() };
    raw.ru_utime = libc::timeval {
        tv_sec: 1,
        tv_usec: 12345,
    };
    raw.ru_stime = libc::timeval {
        tv_sec: 2,
        tv_usec: 34567,
    };
    let t = getrusage::CpuTimes::from_rusage(&raw);
    assert_eq!(t.utime, 1_012_345);
    assert_eq!(t.stime, 2_034_567);
}

/// `wait_pidfd_rusage` reports the reaped child's own CPU times through the
/// production `waitid(P_PIDFD)` path — the mechanism `times` children
/// accounting accumulates.
#[test]
fn wait_pidfd_rusage_reports_child_times() {
    let (_pid, pidfd_opt) = sys::fork_pidfd::fork_pidfd().unwrap();
    match pidfd_opt {
        None => {
            // Child: burn a little un-foldable CPU so its reported user time
            // is non-zero in debug and release builds alike.
            burn_cpu(50_000_000);
            sys::exit(0);
        }
        Some(pidfd) => {
            // Parent: reap via the production path and read the child's times.
            let (status, times) = pidfd.wait_pidfd_rusage().unwrap();
            assert_eq!(status.exit_code(), 0);
            assert!(times.utime > 0, "child must report user time: {times:?}");
        }
    }
}
