#![allow(clippy::unwrap_used)]

use sys::clock::{monotonic_us, realtime_us};

/// Three reads are `Ok` and never decrease: the monotonic clock has no
/// backward step.
#[test]
fn monotonic_is_ok_and_non_decreasing() {
    let a = monotonic_us().unwrap();
    let b = monotonic_us().unwrap();
    let c = monotonic_us().unwrap();
    assert!(a > 0, "the monotonic clock must read a positive value: {a}");
    assert!(b >= a, "{b} < {a}");
    assert!(c >= b, "{c} < {b}");
}

/// Sleeping 20 ms advances the clock by at least 10 ms, and the elapsed span
/// stays sane: a unit conversion that under-counts (seconds scaled down,
/// nanoseconds truncated away) fails here. A sleep is the yardstick, not a CPU
/// burn — an optimizing compiler folds a counting loop away (release builds
/// spend ~0 ms in it) so the elapsed time is not a property of the clock.
#[test]
fn monotonic_advances_across_a_sleep() {
    let start = monotonic_us().unwrap();
    let request = libc::timespec {
        tv_sec: 0,
        tv_nsec: 20_000_000,
    };
    let mut remainder = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: both are valid, aligned, initialized `timespec` values;
    // `nanosleep` only writes through the second one (the time not yet slept).
    let rc = unsafe { libc::nanosleep(&request, &mut remainder) };
    assert_eq!(rc, 0, "nanosleep must succeed");
    let end = monotonic_us().unwrap();
    assert!(
        end - start >= 10_000,
        "the sleep must take at least 10 ms: start={start} end={end}"
    );
    assert!(
        end - start < 5_000_000,
        "the clock must not over-count: start={start} end={end}"
    );
}

/// The `timespec` conversion: seconds scaled by 1e6, nanoseconds truncated to
/// whole microseconds.
#[test]
fn timespec_fields_convert_to_microseconds() {
    let ts = libc::timespec {
        tv_sec: 2,
        tv_nsec: 1_234_567,
    };
    assert_eq!(sys::clock::us(&ts), 2_001_234);
    let zero = libc::timespec {
        tv_sec: 0,
        tv_nsec: 999,
    };
    assert_eq!(sys::clock::us(&zero), 0);
}

/// The realtime clock is the wall clock: microseconds since the epoch, so it
/// reads far past 1e12 (a stubbed `Ok(0)`/`Ok(1)` fails here), two reads never
/// decrease, and the wrapper agrees with a direct libc read within a second.
#[test]
fn realtime_is_wall_clock() {
    let a = realtime_us().unwrap();
    let b = realtime_us().unwrap();
    assert!(a > 1_000_000_000_000, "{a}");
    assert!(b >= a, "{b} < {a}");
    let mut ts = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: `CLOCK_REALTIME` is a legal clock id and `ts` is writable memory
    // of exactly the size the kernel writes.
    let rc = unsafe { libc::clock_gettime(libc::CLOCK_REALTIME, &mut ts) };
    assert_eq!(rc, 0, "the direct libc read must succeed");
    let want = sys::clock::us(&ts);
    assert!(a.abs_diff(want) < 1_000_000, "{a} vs {want}");
}

/// The wall clock (µs since 1970) runs ahead of the boot-based monotonic clock
/// on any machine that booted in the past: the gap between the two is the
/// uptime, and a stubbed or swapped conversion erases it.
#[test]
fn realtime_runs_ahead_of_monotonic() {
    assert!(realtime_us().unwrap() > monotonic_us().unwrap());
}
