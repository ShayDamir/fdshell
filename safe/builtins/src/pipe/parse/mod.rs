use core::ffi::CStr;
use sys::errno::EINVAL;

pub struct PipeConfig;

pub fn pipe_parse(args: &[&CStr]) -> Result<PipeConfig, i32> {
    if args.is_empty() || crate::argparse::wants_help(args) {
        return Err(0);
    }
    Err(EINVAL)
}
