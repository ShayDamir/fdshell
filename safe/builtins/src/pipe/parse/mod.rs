mod flags;

use core::ffi::CStr;
use error_stack::{Report, ResultExt};

use crate::error::{BuiltinError, Suggestion};

pub struct PipeConfig {
    pub flags: i32,
}

/// Parses pipe CLI arguments into a [`PipeConfig`].
pub fn pipe_parse(args: &[&CStr]) -> Result<PipeConfig, Report<BuiltinError>> {
    if crate::argparse::wants_help(args) {
        return Err(Report::new(BuiltinError::Help));
    }
    if args.is_empty() {
        return Ok(PipeConfig { flags: 0 });
    }
    let mut result = PipeConfig { flags: 0 };
    let mut i = 0;
    while i < args.len() {
        let arg = args.get(i).ok_or(BuiltinError::InvalidArgument("arg"))?;
        i += 1;
        let (key, val) = crate::argparse::split(arg)?;
        match key {
            b"--flags" => {
                let v = crate::argparse::next_val(args, &mut i, val)?;
                result.flags |= flags::parse_pipe_flag(v)
                    .change_context(BuiltinError::InvalidArgument("flags"))
                    .attach_opaque(Suggestion(
                        "Use O_NONBLOCK or O_DIRECT, or a hex value (e.g. 0x2)",
                    ))?;
            }
            a if a.starts_with(b"-") => {
                return Err(Report::new(BuiltinError::InvalidArgument("flag")));
            }
            _ => return Err(Report::new(BuiltinError::InvalidArgument("arg"))),
        }
    }
    Ok(result)
}
