#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
use super::*;

/// The report's numbers are `end - start` for each field: the child-time
/// totals carried in both snapshots cancel out of the delta, and a value that
/// went backwards saturates to 0 instead of wrapping.
#[test]
fn elapsed_is_the_snapshot_difference() {
    let start = Start {
        real: 1_000,
        user: 1_000_000,
        sys: 2_000_000,
    };
    let end = Start {
        real: 5_000,
        user: 1_000_000,
        sys: 3_000_000,
    };
    let elapsed = start.elapsed(&end);
    assert_eq!(
        (elapsed.real, elapsed.user, elapsed.sys),
        (4_000, 0, 1_000_000)
    );
    let back = start.elapsed(&Start {
        real: 900,
        user: 999,
        sys: 999,
    });
    assert_eq!((back.real, back.user, back.sys), (0, 0, 0));
}
