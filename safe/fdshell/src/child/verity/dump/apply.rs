//! `--dump`/`--offset`/`--length` flag dispatch onto the in-progress
//! `Option<DumpSpec>` (the only keys `parse_flags` forwards here).

use builtins::error::BuiltinError;
use error_stack::{Report, bail, ensure};
use sys::ShortCStr;

use crate::child::flags::flag_val;

use super::{DumpSpec, parse_metadata_type, parse_uint};

pub(crate) fn apply(
    key: &[u8],
    val: Option<&[u8]>,
    args: &[ShortCStr],
    i: &mut usize,
    spec: &mut Option<DumpSpec>,
) -> Result<(), Report<BuiltinError>> {
    match key {
        b"--dump" => {
            ensure!(spec.is_none(), BuiltinError::InvalidArgument("--dump"));
            let r#type = parse_metadata_type(flag_val(val, args, i, "--dump")?)?;
            *spec = Some(DumpSpec {
                r#type,
                offset: 0,
                length: None,
            });
        }
        b"--offset" => {
            let s = spec_mut(spec, "--offset")?;
            ensure!(s.offset == 0, BuiltinError::InvalidArgument("--offset"));
            s.offset = parse_uint(flag_val(val, args, i, "--offset")?, "--offset")?;
        }
        b"--length" => {
            let s = spec_mut(spec, "--length")?;
            ensure!(
                s.length.is_none(),
                BuiltinError::InvalidArgument("--length")
            );
            s.length = Some(parse_uint(flag_val(val, args, i, "--length")?, "--length")?);
        }
        // `parse_flags` only forwards the three keys above: anything else is
        // an invariant violation, not a user error (§4.10).
        _ => bail!(BuiltinError::Never),
    }
    Ok(())
}

fn spec_mut<'a>(
    spec: &'a mut Option<DumpSpec>,
    flag: &'static str,
) -> Result<&'a mut DumpSpec, Report<BuiltinError>> {
    spec.as_mut()
        .ok_or_else(|| BuiltinError::InvalidArgument(flag).into())
}
