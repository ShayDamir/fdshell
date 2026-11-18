#![allow(clippy::unwrap_used)]

use alloc::{vec, vec::Vec};

use crate::repl::prompt::segment::ValueEscape;
use crate::repl::prompt::value::Values;
use crate::repl::prompt::value::clock::ClockValues;
use sys::SyscallError;

type Instant = fn() -> Result<u64, SyscallError>;

// One fn per pinned instant — the provider's injected clock is a `fn()` field
// (the `run_dispatch/loop_ctl.rs` precedent), which is what makes these exact
// byte pins deterministic.

fn at_1970_01_01_000000() -> Result<u64, SyscallError> {
    Ok(0)
}

fn at_2026_06_15_000709() -> Result<u64, SyscallError> {
    Ok(1_781_482_029_000_000)
}

fn at_2026_11_08_090000() -> Result<u64, SyscallError> {
    Ok(1_794_128_400_000_000)
}

fn at_2000_02_29_120000() -> Result<u64, SyscallError> {
    Ok(951_825_600_000_000)
}

fn at_2024_02_29_235959() -> Result<u64, SyscallError> {
    Ok(1_709_251_199_000_000)
}

fn at_2038_01_19_031407() -> Result<u64, SyscallError> {
    Ok(2_147_483_647_000_000)
}

fn at_2026_11_18_134142() -> Result<u64, SyscallError> {
    Ok(1_795_009_302_000_000)
}

fn at_2100_03_01_000000() -> Result<u64, SyscallError> {
    Ok(4_107_542_400_000_000)
}

fn at_2026_11_15_000000() -> Result<u64, SyscallError> {
    Ok(1_794_700_800_000_000)
}

fn at_2026_11_16_000000() -> Result<u64, SyscallError> {
    Ok(1_794_787_200_000_000)
}

fn at_2026_11_17_000000() -> Result<u64, SyscallError> {
    Ok(1_794_873_600_000_000)
}

fn at_2026_11_18_000000() -> Result<u64, SyscallError> {
    Ok(1_794_960_000_000_000)
}

fn at_2026_11_19_000000() -> Result<u64, SyscallError> {
    Ok(1_795_046_400_000_000)
}

fn at_2026_11_20_000000() -> Result<u64, SyscallError> {
    Ok(1_795_132_800_000_000)
}

fn at_2026_11_21_000000() -> Result<u64, SyscallError> {
    Ok(1_795_219_200_000_000)
}

fn at_2026_01_01_000000() -> Result<u64, SyscallError> {
    Ok(1_767_225_600_000_000)
}

fn at_2026_02_01_000000() -> Result<u64, SyscallError> {
    Ok(1_769_904_000_000_000)
}

fn at_2026_03_01_000000() -> Result<u64, SyscallError> {
    Ok(1_772_323_200_000_000)
}

fn at_2026_04_01_000000() -> Result<u64, SyscallError> {
    Ok(1_775_001_600_000_000)
}

fn at_2026_05_01_000000() -> Result<u64, SyscallError> {
    Ok(1_777_593_600_000_000)
}

fn at_2026_06_01_000000() -> Result<u64, SyscallError> {
    Ok(1_780_272_000_000_000)
}

fn at_2026_07_01_000000() -> Result<u64, SyscallError> {
    Ok(1_782_864_000_000_000)
}

fn at_2026_08_01_000000() -> Result<u64, SyscallError> {
    Ok(1_785_542_400_000_000)
}

fn at_2026_09_01_000000() -> Result<u64, SyscallError> {
    Ok(1_788_220_800_000_000)
}

fn at_2026_10_01_000000() -> Result<u64, SyscallError> {
    Ok(1_790_812_800_000_000)
}

fn at_2026_11_01_000000() -> Result<u64, SyscallError> {
    Ok(1_793_491_200_000_000)
}

fn at_2026_12_01_000000() -> Result<u64, SyscallError> {
    Ok(1_796_083_200_000_000)
}

fn a_clock_that_fails() -> Result<u64, SyscallError> {
    Err(SyscallError::EINVAL("clock_gettime"))
}

fn provider(now: Instant) -> ClockValues {
    ClockValues { now }
}

/// Every escape's answer at one injected instant, in table order — the rows
/// are the glibc-`strftime` reference (UTC, C locale) measured in the plan.
fn all_five(now: Instant) -> Vec<Vec<u8>> {
    let provider = provider(now);
    [
        ValueEscape::Date,
        ValueEscape::Time,
        ValueEscape::Time24,
        ValueEscape::AmPmTime,
        ValueEscape::MinuteTime,
    ]
    .into_iter()
    .map(|esc| provider.value(esc).unwrap())
    .collect()
}

fn date_at(now: Instant) -> Vec<u8> {
    provider(now).value(ValueEscape::Date).unwrap()
}

/// The measured bash 5.3.9 shapes, asserted as **fdshell's** answer (the
/// manual disagrees on `\d` and `\@`, and `\t`/`\T` swap the 24- and 12-hour
/// spellings against their manual lines; LESSONS).
#[test]
fn a_fixed_instant_pins_every_escape() {
    assert_eq!(
        all_five(at_2026_11_18_134142),
        vec![
            b"Wed Nov 18".to_vec(),
            b"13:41:42".to_vec(),
            b"01:41:42".to_vec(),
            b"01:41 PM".to_vec(),
            b"13:41".to_vec(),
        ]
    );
}

#[test]
fn the_pad_and_12_hour_rules() {
    // The epoch row pins the weekday anchor (1970-01-01 is a Thursday), the
    // `%e` space pad and the midnight half of the 12-hour rule (`12:00 AM`).
    assert_eq!(
        all_five(at_1970_01_01_000000),
        vec![
            b"Thu Jan  1".to_vec(),
            b"00:00:00".to_vec(),
            b"12:00:00".to_vec(),
            b"12:00 AM".to_vec(),
            b"00:00".to_vec(),
        ]
    );
    assert_eq!(
        all_five(at_2026_06_15_000709),
        vec![
            b"Mon Jun 15".to_vec(),
            b"00:07:09".to_vec(),
            b"12:07:09".to_vec(),
            b"12:07 AM".to_vec(),
            b"00:07".to_vec(),
        ]
    );
    assert_eq!(
        all_five(at_2026_11_08_090000),
        vec![
            b"Sun Nov  8".to_vec(),
            b"09:00:00".to_vec(),
            b"09:00:00".to_vec(),
            b"09:00 AM".to_vec(),
            b"09:00".to_vec(),
        ]
    );
    // Noon is the other half of the 12-hour rule (`12:00 PM`) and the era's
    // leap arithmetic (`2000-02-29` is the last day before the 2000-03-01
    // era boundary).
    assert_eq!(
        all_five(at_2000_02_29_120000),
        vec![
            b"Tue Feb 29".to_vec(),
            b"12:00:00".to_vec(),
            b"12:00:00".to_vec(),
            b"12:00 PM".to_vec(),
            b"12:00".to_vec(),
        ]
    );
    assert_eq!(
        all_five(at_2024_02_29_235959),
        vec![
            b"Thu Feb 29".to_vec(),
            b"23:59:59".to_vec(),
            b"11:59:59".to_vec(),
            b"11:59 PM".to_vec(),
            b"23:59".to_vec(),
        ]
    );
    // The `time_t` boundary row (the arithmetic is `i64`; the row outlives
    // bash's 32-bit ancestors unchanged).
    assert_eq!(
        all_five(at_2038_01_19_031407),
        vec![
            b"Tue Jan 19".to_vec(),
            b"03:14:07".to_vec(),
            b"03:14:07".to_vec(),
            b"03:14 AM".to_vec(),
            b"03:14".to_vec(),
        ]
    );
}

#[test]
fn every_weekday_and_month_name_is_pinned() {
    // One instant per weekday and one per month: every table index is named,
    // so each index-carrying expression has a kill. `2100-03-01` is the
    // non-leap-century row. Only `\d` is asserted here; the rows above pin
    // full five-column shapes.
    for (now, expected) in WEEKDAY_ROWS {
        assert_eq!(date_at(now), expected.as_bytes(), "{expected:?}");
    }
    for (now, expected) in MONTH_ROWS {
        assert_eq!(date_at(now), expected.as_bytes(), "{expected:?}");
    }
}

const WEEKDAY_ROWS: [(Instant, &str); 7] = [
    (at_2026_11_15_000000, "Sun Nov 15"),
    (at_2026_11_16_000000, "Mon Nov 16"),
    (at_2026_11_17_000000, "Tue Nov 17"),
    (at_2026_11_18_000000, "Wed Nov 18"),
    (at_2026_11_19_000000, "Thu Nov 19"),
    (at_2026_11_20_000000, "Fri Nov 20"),
    (at_2026_11_21_000000, "Sat Nov 21"),
];

const MONTH_ROWS: [(Instant, &str); 13] = [
    (at_2026_01_01_000000, "Thu Jan  1"),
    (at_2026_02_01_000000, "Sun Feb  1"),
    (at_2026_03_01_000000, "Sun Mar  1"),
    (at_2026_04_01_000000, "Wed Apr  1"),
    (at_2026_05_01_000000, "Fri May  1"),
    (at_2026_06_01_000000, "Mon Jun  1"),
    (at_2026_07_01_000000, "Wed Jul  1"),
    (at_2026_08_01_000000, "Sat Aug  1"),
    (at_2026_09_01_000000, "Tue Sep  1"),
    (at_2026_10_01_000000, "Thu Oct  1"),
    (at_2026_11_01_000000, "Sun Nov  1"),
    (at_2026_12_01_000000, "Tue Dec  1"),
    (at_2100_03_01_000000, "Mon Mar  1"),
];

#[test]
fn a_failing_clock_answers_no_value() {
    // The `.ok()?` arm at its mapping boundary: a clock read that fails is a
    // source that failed, so every escape answers `None` — decision D2's "no
    // value", which the renderer draws as the placeholder's literal text.
    let provider = provider(a_clock_that_fails);
    for esc in [
        ValueEscape::Date,
        ValueEscape::Time,
        ValueEscape::Time24,
        ValueEscape::AmPmTime,
        ValueEscape::MinuteTime,
    ] {
        assert_eq!(provider.value(esc), None, "{esc:?}");
    }
}

#[test]
fn the_lookup_fallback_is_pinned() {
    // The `get` fallbacks of the two tables are pinned behaviour, not an
    // unreachable arm: an out-of-range index draws the empty name.
    assert_eq!(super::weekday_name(7), "");
    assert_eq!(super::month_name(12), "");
}
