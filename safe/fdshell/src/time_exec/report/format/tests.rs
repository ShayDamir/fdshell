#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
use super::*;
use crate::time_exec::report::Start;

#[test]
fn centis_truncates_to_hundredths() {
    assert_eq!(centis(0), "0.00");
    assert_eq!(centis(9_999), "0.99");
    assert_eq!(centis(10_000), "1.00");
    assert_eq!(centis(123_456), "12.34");
}

#[test]
fn seconds_is_the_posix_unit() {
    // `time -p` prints seconds, not centiseconds: a tenth of a second is 0.10.
    assert_eq!(seconds(0), "0.00");
    assert_eq!(seconds(100_000), "0.10");
    assert_eq!(seconds(999_999), "0.99");
    assert_eq!(seconds(1_000_000), "1.00");
    assert_eq!(seconds(30_051_000), "30.05");
}

#[test]
fn clock_is_bashs_minutes_layout() {
    assert_eq!(clock(0), "0m0.000s");
    assert_eq!(clock(1_000_000), "0m1.000s");
    assert_eq!(clock(104_000), "0m0.104s");
    assert_eq!(clock(60_000_000), "1m0.000s");
    assert_eq!(clock(61_234_567), "1m1.234s");
}

#[test]
fn posix_report_is_the_three_line_layout() {
    let out = report(true, 104_000, 20_000, 10_000);
    assert_eq!(out, "real 0.10\nuser 0.02\nsys 0.01\n");
}

#[test]
fn default_report_leads_with_a_blank_line() {
    let out = report(false, 104_000, 2_000, 1_000);
    assert_eq!(out, "\nreal\t0m0.104s\nuser\t0m0.002s\nsys\t0m0.001s\n");
}

#[test]
fn zero_snapshot_reports_zeros() {
    assert_eq!(report(true, 0, 0, 0), "real 0.00\nuser 0.00\nsys 0.00\n");
    assert_eq!(
        report(false, 0, 0, 0),
        "\nreal\t0m0.000s\nuser\t0m0.000s\nsys\t0m0.000s\n"
    );
}

/// The two layouts of a real delta, so the report text is pinned end to end.
#[test]
fn elapsed_report_layouts() {
    let start = Start {
        real: 0,
        user: 0,
        sys: 0,
    };
    let end = Start {
        real: 104_000,
        user: 20_000,
        sys: 10_000,
    };
    let elapsed = start.elapsed(&end);
    assert_eq!(
        report(true, elapsed.real, elapsed.user, elapsed.sys),
        "real 0.10\nuser 0.02\nsys 0.01\n"
    );
    assert_eq!(
        report(false, elapsed.real, elapsed.user, elapsed.sys),
        "\nreal\t0m0.104s\nuser\t0m0.020s\nsys\t0m0.010s\n"
    );
}
