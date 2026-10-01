use core::ffi::CStr;
use error_stack::{Report, ResultExt};

use crate::error::TimeParseError;

/// A timestamp spec for `utimensat`: `now`, `omit`, or an epoch-seconds value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimeSpec {
    /// `UTIME_NOW` — set to the current time.
    Now,
    /// `UTIME_OMIT` — leave this timestamp untouched.
    Omit,
    /// A literal epoch-seconds value (negative = pre-1970); nanoseconds are 0.
    Epoch(i64),
}

impl TimeSpec {
    /// The uapi `timespec` encoding. The kernel requires `tv_sec == 0` for the
    /// special values, with the marker living in `tv_nsec`.
    pub fn to_timespec(self) -> sys::fileat::Timespec {
        match self {
            TimeSpec::Now => sys::fileat::Timespec {
                tv_sec: 0,
                tv_nsec: sys::fileat::UTIME_NOW,
            },
            TimeSpec::Omit => sys::fileat::Timespec {
                tv_sec: 0,
                tv_nsec: sys::fileat::UTIME_OMIT,
            },
            TimeSpec::Epoch(sec) => sys::fileat::Timespec {
                tv_sec: sec,
                tv_nsec: 0,
            },
        }
    }
}

/// Parses a time spec: `now`, `omit`, or a decimal epoch-seconds integer.
pub(crate) fn parse_time_spec(s: &CStr) -> Result<TimeSpec, Report<TimeParseError>> {
    let b = s.to_bytes();
    if b == b"now" {
        return Ok(TimeSpec::Now);
    }
    if b == b"omit" {
        return Ok(TimeSpec::Omit);
    }
    let text = core::str::from_utf8(b).change_context(TimeParseError::Utf8)?;
    text.parse::<i64>()
        .map(TimeSpec::Epoch)
        .change_context(TimeParseError::ParseFailed)
}
