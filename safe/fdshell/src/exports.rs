use crate::error::exports::ExportError;
use crate::state::ShellState;
use alloc::vec::Vec;
use error_stack::{Report, ResultExt};

use sys::{ShortCStr, ShortCStrError};

pub fn handle_export(
    args: &[ShortCStr],
    state: &mut ShellState,
) -> Result<(), Report<ExportError>> {
    match args.first() {
        None => {
            list_exports(state)?;
            Ok(())
        }
        Some(arg) => {
            if let Some((name, value)) = arg.split_once_byte(b'=') {
                set_export(name, value, state).change_context(ExportError::NulByte)?;
            } else {
                mark_exported(arg, state).change_context(ExportError::NulByte)?;
            }
            Ok(())
        }
    }
}

fn list_exports(state: &ShellState) -> Result<(), Report<ExportError>> {
    for (k, v) in &state.exports {
        let key_bytes = k.as_bytes().change_context(ExportError::Never)?;
        let mut line: Vec<u8> = b"export ".to_vec();
        line.extend_from_slice(key_bytes);
        line.extend_from_slice(b"=");
        line.extend_from_slice(v.as_slice());
        line.push(b'\n');
        sys::OUT.write_all(&line).change_context(ExportError::Io)?;
    }
    Ok(())
}

fn set_export(
    name: ShortCStr,
    value: ShortCStr,
    state: &mut ShellState,
) -> Result<(), Report<ExportError>> {
    let val_bytes = value
        .as_bytes()
        .change_context(ExportError::NulByte)?
        .to_vec();
    state.exports.insert(name.clone(), val_bytes.clone());
    state.strings.insert(
        name,
        ShortCStr::from_vec(val_bytes).change_context(ExportError::Never)?,
    );
    Ok(())
}

fn mark_exported(arg: &ShortCStr, state: &mut ShellState) -> Result<(), ShortCStrError> {
    let name_bytes = arg.as_bytes()?.to_vec();
    state.exports.insert(arg.clone(), Vec::new());
    // Also set the corresponding shell variable.
    let _var_name =
        alloc::ffi::CString::new(name_bytes.clone()).map_err(|_| ShortCStrError::NulByte)?;
    Ok(())
}
