// mutants: -C --lib -C --test -C repl -- -E 'test(/^repl::/) | binary(repl)'

//! Task #618's provider: the values of `\d \t \T \@ \A`, answered from the
//! `CLOCK_REALTIME` wrapper (#614) through the seam.
//!
//! fdshell formats in **UTC** at the **C locale**: a `no_std` shell cannot
//! reach tzdata, and the safe crates never call libc, so the answer is
//! byte-identical to what bash prints under `TZ=UTC LC_ALL=C` — on this box
//! bash itself runs through `localtime` (CET), which shifts the clock by the
//! local offset and can move `\d` a whole day near midnight. `%p` is
//! uppercase `AM`/`PM` — the C-locale spelling bash prints under `LC_ALL=C`
//! (this box's `en_GB.UTF-8` lowercases it; a test must not depend on the
//! machine's locale). The two formats bash measures differently from its
//! manual — `\d` is `%a %b %e` (no time tail), `\@` is `%I:%M %p` **with** the
//! space, and `\T` answers the 12-hour spelling while `\t` answers the 24-hour
//! one — are pinned as fdshell's own answer, measured against bash 5.3.9 on
//! 2026-11-18 (LESSONS: "A test that measures wall-clock under load cannot
//! pin the number").
//!
//! The clock source is the injected `fn()` field — production value
//! `sys::clock::realtime_us`, tests pin the formats at fixed instants. One
//! clock read per escape, so a prompt naming no clock escape pays no syscall,
//! and five escapes straddling a second boundary may disagree (the end-to-end
//! row pins shapes only). A clock read that fails answers `None` — decision
//! D2's "no value", drawn as the placeholder's literal text.

pub(crate) mod civil;

use alloc::format;
use alloc::vec::Vec;

use crate::repl::prompt::segment::ValueEscape;
use crate::repl::prompt::value::Values;
use crate::repl::prompt::value::clock::civil::Civil;
use sys::SyscallError;

/// The five clock escapes, answered through the seam one escape at a time
/// (decision D3) like `shell.rs` answers its family.
pub(crate) struct ClockValues {
    now: fn() -> Result<u64, SyscallError>,
}

impl ClockValues {
    pub(crate) fn new() -> Self {
        Self {
            now: sys::clock::realtime_us,
        }
    }
}

impl Values for ClockValues {
    fn value(&self, esc: ValueEscape) -> Option<Vec<u8>> {
        if !matches!(
            esc,
            ValueEscape::Date
                | ValueEscape::Time
                | ValueEscape::Time24
                | ValueEscape::AmPmTime
                | ValueEscape::MinuteTime
        ) {
            return None;
        }
        let now = Civil::from_us((self.now)().ok()?);
        match esc {
            ValueEscape::Date => Some(
                format!(
                    "{} {} {:>2}",
                    weekday_name(now.weekday),
                    month_name(now.month - 1),
                    now.day
                )
                .into_bytes(),
            ),
            ValueEscape::Time => {
                Some(format!("{:02}:{:02}:{:02}", now.hour, now.minute, now.second).into_bytes())
            }
            ValueEscape::Time24 => {
                Some(format!("{:02}:{:02}:{:02}", now.hour12, now.minute, now.second).into_bytes())
            }
            ValueEscape::AmPmTime => Some(
                format!(
                    "{:02}:{:02} {}",
                    now.hour12,
                    now.minute,
                    if now.is_pm { "PM" } else { "AM" }
                )
                .into_bytes(),
            ),
            _ => Some(format!("{:02}:{:02}", now.hour, now.minute).into_bytes()),
        }
    }
}

/// `%a`, the C-locale weekday names in table order (`Sun..Sat`); 1970-01-01
/// is a Thursday, pinned by the epoch row. Out of range draws the fallback,
/// pinned by `the_lookup_fallback_is_pinned`.
fn weekday_name(index: usize) -> &'static str {
    WEEKDAYS.get(index).copied().unwrap_or_default()
}

/// `%b`, the C-locale month names; same fallback posture as `weekday_name`.
fn month_name(index: usize) -> &'static str {
    MONTHS.get(index).copied().unwrap_or_default()
}

const WEEKDAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

#[cfg(test)]
mod tests;
