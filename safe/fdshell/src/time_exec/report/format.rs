//! The `time` report's **text**: the two layouts and the three unit
//! formatters. Extracted from `time_exec/report.rs` for the line budget
//! (task #456); the clock/rusage snapshots stay in the parent.

use alloc::format;
use alloc::string::String;

/// The report text: the POSIX `-p` layout, or bash's default (a leading empty
/// line, tab-separated `N m N.NNN s` values).
pub(in crate::time_exec::report) fn report(
    posix: bool,
    real: u64,
    user: u64,
    sys_us: u64,
) -> String {
    if posix {
        return format!(
            "real {}\nuser {}\nsys {}\n",
            seconds(real),
            seconds(user),
            seconds(sys_us)
        );
    }
    format!(
        "\nreal\t{}\nuser\t{}\nsys\t{}\n",
        clock(real),
        clock(user),
        clock(sys_us)
    )
}

/// Microseconds as bash's `N.NN` centiseconds (the unit the `times` builtin
/// prints, which pins the same conversion).
pub(crate) fn centis(us: u64) -> String {
    format!("{}.{:02}", us / 10_000, (us / 100) % 100)
}

/// Microseconds as the POSIX `time -p` unit: whole seconds plus two fractional
/// digits (`real 0.10` for a tenth of a second).
fn seconds(us: u64) -> String {
    format!("{}.{:02}", us / 1_000_000, (us / 10_000) % 100)
}

/// Microseconds as bash's default-layout `N m N.NNN s` (integer math).
fn clock(us: u64) -> String {
    format!(
        "{}m{}.{:03}s",
        us / 60_000_000,
        (us / 1_000_000) % 60,
        (us / 1_000) % 1_000
    )
}

#[cfg(test)]
mod tests;
