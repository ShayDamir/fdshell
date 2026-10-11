#![allow(clippy::unwrap_used)]

use sys::clock::monotonic_us;

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
