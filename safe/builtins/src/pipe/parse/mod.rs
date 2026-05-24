mod flags;

use core::ffi::CStr;
use sys::errno::EINVAL;

pub struct PipeConfig {
    pub flags: i32,
}

/// Parses pipe CLI arguments into a [`PipeConfig`].
///
/// Returns:
/// - `Err(sys::errno::HELP)` — `--help` or `-h` was passed
/// - `Err(sys::errno::EINVAL)` — unknown flag, bad hex, etc.
pub fn pipe_parse(args: &[&CStr]) -> Result<PipeConfig, i32> {
    if args.is_empty() || crate::argparse::wants_help(args) {
        return Err(sys::errno::HELP);
    }
    let mut result = PipeConfig { flags: 0 };
    let mut i = 0;
    while i < args.len() {
        let arg = args.get(i).ok_or(EINVAL)?;
        i += 1;
        let (key, val) = crate::argparse::split(arg)?;
        match key {
            b"--flags" => {
                let v = crate::argparse::next_val(args, &mut i, val)?;
                result.flags |= flags::parse_pipe_flag(v)?;
            }
            a if a.starts_with(b"-") => return Err(EINVAL),
            _ => return Err(EINVAL),
        }
    }
    Ok(result)
}
