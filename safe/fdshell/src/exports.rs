use crate::error::exports::ExportError;
use crate::state::ShellState;
use error_stack::{Report, ResultExt};
use sys::{ShortCStr, ShortCStrError};

pub fn handle_export(
    args: &[ShortCStr],
    state: &mut ShellState,
) -> Result<(), Report<ExportError>> {
    match args.first() {
        None => {
            list_exports(state);
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

fn list_exports(state: &ShellState) {
    for (k, v) in &state.exports {
        let key_str = core::str::from_utf8(k.as_bytes().unwrap_or(&[])).unwrap_or("?");
        let val_str = core::str::from_utf8(v.as_slice()).unwrap_or("?");
        println!("export {}={}", key_str, val_str);
    }
}

fn set_export(
    name: ShortCStr,
    value: ShortCStr,
    state: &mut ShellState,
) -> Result<(), ShortCStrError> {
    let name_bytes = name.as_bytes()?.to_vec();
    let val_bytes = value.as_bytes()?.to_vec();
    state.strings.insert(
        ShortCStr::from_vec(name_bytes.clone())?,
        ShortCStr::from_vec(val_bytes.clone())?,
    );
    state
        .exports
        .insert(ShortCStr::from_vec(name_bytes)?, val_bytes);
    Ok(())
}

fn mark_exported(arg: &ShortCStr, state: &mut ShellState) -> Result<(), ShortCStrError> {
    let target = arg.as_bytes()?;
    if let Some(val) = state.strings.get(arg) {
        state.exports.insert(
            ShortCStr::from_vec(target.to_vec())?,
            val.as_bytes()?.to_vec(),
        );
    } else {
        // No existing string var — store as both with empty value
        state
            .strings
            .insert(ShortCStr::from_vec(target.to_vec())?, ShortCStr::new());
        state
            .exports
            .insert(ShortCStr::from_vec(target.to_vec())?, Vec::new());
    }
    Ok(())
}
