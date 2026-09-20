use core::ffi::CStr;
use error_stack::{Report, bail};

use crate::error::BuiltinError;

pub struct ExecFdConfig<'a> {
    pub var: &'a CStr,
}

/// Parses exec_fd CLI arguments into an [`ExecFdConfig`].
///
/// Arguments must be the original (unsubstituted) words, so the `%var`
/// reference is intact; everything after the var belongs to the program.
pub fn execfd_parse<'a>(args: &'a [&'a CStr]) -> Result<ExecFdConfig<'a>, Report<BuiltinError>> {
    if args.is_empty() || crate::argparse::wants_help(args) {
        bail!(BuiltinError::Help);
    }

    let var = args.first().ok_or(BuiltinError::InvalidArgument("var"))?;
    let name = var.to_bytes();
    if !name.starts_with(b"%") || name.len() == 1 {
        bail!(BuiltinError::InvalidArgument("var"));
    }

    Ok(ExecFdConfig { var })
}
