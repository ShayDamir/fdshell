mod flags;

use core::ffi::CStr;
use error_stack::{Report, ResultExt, bail};

use crate::error::{BuiltinError, Suggestion};

/// Parsed `eventfd` arguments: the initial counter and flags.
pub struct EventfdConfig {
    pub init: u32,
    pub flags: i32,
}

/// Parses `eventfd [init] [--flags F]`.
pub fn eventfd_parse(args: &[&CStr]) -> Result<EventfdConfig, Report<BuiltinError>> {
    if crate::argparse::wants_help(args) {
        bail!(BuiltinError::Help);
    }
    let mut init: Option<u32> = None;
    let mut result_flags: i32 = 0;
    let mut i = 0;
    while i < args.len() {
        let arg = args.get(i).ok_or(BuiltinError::InvalidArgument("arg"))?;
        i += 1;
        let (key, val) = crate::argparse::split(arg)?;
        match key {
            b"--flags" => {
                let v = crate::argparse::next_val(args, &mut i, val)?;
                result_flags |= flags::parse_eventfd_flag(v)
                    .change_context(BuiltinError::InvalidArgument("flags"))
                    .attach_opaque(Suggestion(
                        "Use EFD_NONBLOCK, EFD_SEMAPHORE, or a hex value (e.g. 0x800)",
                    ))?;
            }
            a if a.starts_with(b"-") => bail!(BuiltinError::InvalidArgument("flag")),
            _ => {
                if init.is_none() {
                    init = Some(parse_init(arg)?);
                } else {
                    bail!(BuiltinError::InvalidArgument("arg"));
                }
            }
        }
    }
    let init = init.unwrap_or(0);
    Ok(EventfdConfig {
        init,
        flags: result_flags,
    })
}

fn parse_init(s: &CStr) -> Result<u32, Report<BuiltinError>> {
    let b = s.to_bytes();
    let n = core::str::from_utf8(b).change_context(BuiltinError::InvalidArgument("init"))?;
    n.parse::<u32>()
        .change_context(BuiltinError::InvalidArgument("init"))
}
