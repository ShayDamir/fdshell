#![allow(clippy::unwrap_used)]

use crate::repl::prompt::value::clock::civil::Civil;

/// Each row is a full struct literal, so every field — the day-of-week offset,
/// the era constant, the `mp < 10` month shift, the leap arithmetic and the two
/// derived 12-hour answers — carries a kill.
///
/// The weekday anchor: 1970-01-01 is a Thursday (the `(days + 4)` offset), and
/// midnight is the `12 ... AM` half of the 12-hour rule.
#[test]
fn the_epoch_anchor_is_a_thursday() {
    assert_eq!(
        Civil::from_us(0),
        Civil {
            weekday: 4,
            month: 1,
            day: 1,
            hour: 0,
            minute: 0,
            second: 0,
            hour12: 12,
            is_pm: false,
        }
    );
}

/// Hinnant's eras start on March 1 of a 400-year cycle's first leap year, so
/// `2000-02-29` is the last day of the previous era and `2000-03-01` the first
/// of the next: a mutated era constant or anchor moves one of the two rows
/// (the noon row also pins the `12:00 PM` spelling at the divisor's edge).
#[test]
fn the_era_boundary_turns_over_on_march_first() {
    assert_eq!(
        Civil::from_us(951_825_600_000_000),
        Civil {
            weekday: 2,
            month: 2,
            day: 29,
            hour: 12,
            minute: 0,
            second: 0,
            hour12: 12,
            is_pm: true,
        }
    );
    assert_eq!(
        Civil::from_us(951_868_800_000_000),
        Civil {
            weekday: 3,
            month: 3,
            day: 1,
            hour: 0,
            minute: 0,
            second: 0,
            hour12: 12,
            is_pm: false,
        }
    );
}

/// The leap day rolls over to March at the exact second boundary: the
/// day-of-year and month-shift steps (`mp < 10`) are on the line between the
/// two rows, and the `yoe/4` term is exercised by the leap day itself.
#[test]
fn the_leap_day_rolls_over_to_march() {
    assert_eq!(
        Civil::from_us(1_709_251_199_000_000),
        Civil {
            weekday: 4,
            month: 2,
            day: 29,
            hour: 23,
            minute: 59,
            second: 59,
            hour12: 11,
            is_pm: true,
        }
    );
    assert_eq!(
        Civil::from_us(1_709_251_200_000_000),
        Civil {
            weekday: 5,
            month: 3,
            day: 1,
            hour: 0,
            minute: 0,
            second: 0,
            hour12: 12,
            is_pm: false,
        }
    );
}

/// 2100 is not a leap year (the `yoe/100` term): the century's March 1 lands on
/// the weekday the Gregorian correction gives, a day later than the Julian
/// cycle would.
#[test]
fn the_century_2100_is_not_a_leap_year() {
    assert_eq!(
        Civil::from_us(4_107_542_400_000_000),
        Civil {
            weekday: 1,
            month: 3,
            day: 1,
            hour: 0,
            minute: 0,
            second: 0,
            hour12: 12,
            is_pm: false,
        }
    );
}

/// The `time_t` boundary row of the reference table; the arithmetic is `i64`, so
/// the row is exact, and it is the last pre-2038 pin.
#[test]
fn the_2038_row_keeps_its_fields() {
    assert_eq!(
        Civil::from_us(2_147_483_647_000_000),
        Civil {
            weekday: 2,
            month: 1,
            day: 19,
            hour: 3,
            minute: 14,
            second: 7,
            hour12: 3,
            is_pm: false,
        }
    );
}
