mod flags;

use core::ffi::CStr;
use error_stack::{Report, ResultExt, bail};

use crate::error::{BuiltinError, Suggestion};

/// Parsed `timerfd` arguments: the expiry, the repeat period, and flags.
pub struct TimerfdConfig {
    pub value_sec: i64,
    pub value_nsec: i64,
    pub interval_sec: i64,
    pub interval_nsec: i64,
    pub flags: i32,
}

/// Parses `timerfd <seconds> [nanos] [--periodic] [--flags F]`.
pub fn timerfd_parse(args: &[&CStr]) -> Result<TimerfdConfig, Report<BuiltinError>> {
    if crate::argparse::wants_help(args) {
        bail!(BuiltinError::Help);
    }
    let mut value_sec: Option<i64> = None;
    let mut value_nsec: Option<i64> = None;
    let mut periodic = false;
    let mut result_flags: i32 = 0;
    let mut i = 0;
    while i < args.len() {
        let arg = args.get(i).ok_or(BuiltinError::InvalidArgument("arg"))?;
        i += 1;
        let (key, val) = crate::argparse::split(arg)?;
        match key {
            b"--periodic" => periodic = true,
            b"--flags" => {
                let v = crate::argparse::next_val(args, &mut i, val)?;
                result_flags |= flags::parse_timerfd_flag(v)
                    .change_context(BuiltinError::InvalidArgument("flags"))
                    .attach_opaque(Suggestion("Use TFD_NONBLOCK, or a hex value (e.g. 0x800)"))?;
            }
            a if a.starts_with(b"-") => bail!(BuiltinError::InvalidArgument("flag")),
            _ => {
                if value_sec.is_none() {
                    value_sec = Some(parse_seconds(arg)?);
                } else if value_nsec.is_none() {
                    value_nsec = Some(parse_nanos(arg)?);
                } else {
                    bail!(BuiltinError::InvalidArgument("arg"));
                }
            }
        }
    }
    let value_sec = value_sec.ok_or(BuiltinError::MissingArgument("seconds"))?;
    let value_nsec = value_nsec.unwrap_or(0);
    let (interval_sec, interval_nsec) = if periodic {
        (value_sec, value_nsec)
    } else {
        (0, 0)
    };
    Ok(TimerfdConfig {
        value_sec,
        value_nsec,
        interval_sec,
        interval_nsec,
        flags: result_flags,
    })
}

fn parse_seconds(s: &CStr) -> Result<i64, Report<BuiltinError>> {
    let b = s.to_bytes();
    let n = core::str::from_utf8(b).change_context(BuiltinError::InvalidArgument("seconds"))?;
    let v = n
        .parse::<i64>()
        .change_context(BuiltinError::InvalidArgument("seconds"))?;
    if v < 0 {
        bail!(BuiltinError::InvalidArgument("seconds"));
    }
    Ok(v)
}

fn parse_nanos(s: &CStr) -> Result<i64, Report<BuiltinError>> {
    let b = s.to_bytes();
    let n = core::str::from_utf8(b).change_context(BuiltinError::InvalidArgument("nanos"))?;
    let v = n
        .parse::<i64>()
        .change_context(BuiltinError::InvalidArgument("nanos"))?;
    if !(0..1_000_000_000).contains(&v) {
        bail!(BuiltinError::InvalidArgument("nanos"));
    }
    Ok(v)
}
