// mutants: -C --lib -C --test -C repl -- -E 'test(/^repl::/) | binary(repl)'

//! Epoch microseconds to the broken-down **UTC** time the five clock formats
//! consume: Hinnant's `civil_from_days`, all `i64` `div_euclid`/`rem_euclid` so
//! nothing panics at the era anchor, and the fields stop at exactly what the
//! formats read — no `year`: no format needs it, and an unused field is dead
//! code with unkillable mutants.

/// The six fields the formats read, plus the two derived answers. `weekday` is
/// the `Sun..Sat` table index, `month` the calendar month (`1-12`).
#[derive(Debug, PartialEq)]
pub(crate) struct Civil {
    pub(crate) weekday: usize,
    pub(crate) month: usize,
    pub(crate) day: usize,
    pub(crate) hour: usize,
    pub(crate) minute: usize,
    pub(crate) second: usize,
    pub(crate) hour12: usize,
    pub(crate) is_pm: bool,
}

impl Civil {
    /// Break epoch microseconds down in UTC; sub-second precision is truncated,
    /// the formats draw whole seconds.
    pub(crate) fn from_us(us: u64) -> Self {
        let secs = (us / 1_000_000) as i64;
        let (weekday, month, day) = civil_from_days(secs.div_euclid(86_400));
        let day_secs = secs.rem_euclid(86_400);
        let hour = day_secs.div_euclid(3_600);
        let hour12 = hour.rem_euclid(12);
        Self {
            weekday,
            month,
            day,
            hour: hour as usize,
            minute: day_secs.div_euclid(60).rem_euclid(60) as usize,
            second: day_secs.rem_euclid(60) as usize,
            hour12: if hour12 == 0 { 12 } else { hour12 as usize },
            is_pm: hour >= 12,
        }
    }
}

/// Days since 1970-01-01 to (weekday table index, calendar month, day of month)
/// — the calendar half of `civil_from_days`; the year only feeds the day-of-year
/// step, so it stays inside this function.
fn civil_from_days(days: i64) -> (usize, usize, usize) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe.div_euclid(1_460) + doe.div_euclid(36_524) - doe.div_euclid(146_096))
        .div_euclid(365);
    let doy = doe - (365 * yoe + yoe.div_euclid(4) - yoe.div_euclid(100));
    let mp = (5 * doy + 2).div_euclid(153);
    let day = (doy - (153 * mp + 2).div_euclid(5) + 1) as usize;
    let month = (if mp < 10 { mp + 3 } else { mp - 9 }) as usize;
    let weekday = (days + 4).rem_euclid(7) as usize;
    (weekday, month, day)
}

#[cfg(test)]
mod tests;
