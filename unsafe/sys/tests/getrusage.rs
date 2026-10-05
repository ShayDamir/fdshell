#![allow(clippy::unwrap_used)]

use sys::getrusage;

/// `self_usage()` succeeds and reports a positive own-user time.
#[test]
fn self_usage_succeeds() {
    let t = getrusage::self_usage().unwrap();
    // u64 fields are finite by construction; the process has burned user time
    // by now, so `utime` must be positive.
    assert!(t.utime > 0, "utime must be positive: {t:?}");
}

/// `wait_pidfd_rusage` reports the reaped child's own CPU times through the
/// production `waitid(P_PIDFD)` path — the mechanism `times` children
/// accounting accumulates.
#[test]
fn wait_pidfd_rusage_reports_child_times() {
    let (_pid, pidfd_opt) = sys::fork_pidfd::fork_pidfd().unwrap();
    match pidfd_opt {
        None => {
            // Child: burn a little CPU so its reported user time is non-zero.
            let mut n = 0u64;
            while n < 50_000_000 {
                n = n.wrapping_add(1);
            }
            core::hint::black_box(n);
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
