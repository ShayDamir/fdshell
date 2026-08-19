use core::ffi::CStr;
use error_stack::{Report, bail};

use crate::error::BuiltinError;

pub struct ExecAtConfig<'a> {
    pub var: &'a CStr,
    pub pathname: &'a CStr,
}

/// Parses exec_at CLI arguments into an [`ExecAtConfig`].
///
/// Arguments must be the original (unsubstituted) words, so the `%var`
/// reference and the pathname are intact; everything after the pathname
/// belongs to the program.
pub fn execat_parse<'a>(args: &'a [&'a CStr]) -> Result<ExecAtConfig<'a>, Report<BuiltinError>> {
    if args.is_empty() || crate::argparse::wants_help(args) {
        bail!(BuiltinError::Help);
    }

    let var = args.first().ok_or(BuiltinError::InvalidArgument("var"))?;
    let name = var.to_bytes();
    if !name.starts_with(b"%") || name.len() == 1 {
        bail!(BuiltinError::InvalidArgument("var"));
    }

    let pathname = args
        .get(1)
        .ok_or(BuiltinError::InvalidArgument("pathname"))?;
    if pathname.to_bytes().is_empty() {
        bail!(BuiltinError::InvalidArgument("pathname"));
    }

    Ok(ExecAtConfig { var, pathname })
}
