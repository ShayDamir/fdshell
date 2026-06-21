mod flags;

use core::ffi::CStr;

use crate::error::BuiltinError;

pub struct PipeConfig {
    pub flags: i32,
}

/// Parses pipe CLI arguments into a [`PipeConfig`].
/// Empty args produce a default config (flags=0 → `O_CLOEXEC`).
///
/// Returns:
/// - `Err(BuiltinError::Help)` — `--help` or `-h` was passed
/// - `Err(BuiltinError::InvalidArgument)` — unknown flag, bad hex, etc.
pub fn pipe_parse(args: &[&CStr]) -> Result<PipeConfig, BuiltinError> {
    if crate::argparse::wants_help(args) {
        return Err(BuiltinError::Help);
    }
    if args.is_empty() {
        return Ok(PipeConfig { flags: 0 });
    }
    let mut result = PipeConfig { flags: 0 };
    let mut i = 0;
    while i < args.len() {
        let arg = args.get(i).ok_or(BuiltinError::InvalidArgument)?;
        i += 1;
        let (key, val) = crate::argparse::split(arg)?;
        match key {
            b"--flags" => {
                let v = crate::argparse::next_val(args, &mut i, val)?;
                result.flags |= flags::parse_pipe_flag(v)?;
            }
            a if a.starts_with(b"-") => return Err(BuiltinError::InvalidArgument),
            _ => return Err(BuiltinError::InvalidArgument),
        }
    }
    Ok(result)
}
