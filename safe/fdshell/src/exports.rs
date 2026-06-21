use crate::error::exports::InvalidExportName;
use crate::state::ShellState;
use error_stack::{Report, ResultExt};
use sys::{ShortCStr, ShortCStrError};

pub fn handle_export(
    args: &[ShortCStr],
    state: &mut ShellState,
) -> Result<(), Report<InvalidExportName>> {
    match args.first() {
        None => {
            list_exports(state);
            Ok(())
        }
        Some(arg) => {
            if let Some(eq_pos) = arg
                .as_bytes()
                .change_context(InvalidExportName)?
                .iter()
                .position(|&b| b == b'=')
            {
                set_export(arg, eq_pos, state).change_context(InvalidExportName)?;
            } else {
                mark_exported(arg, state).change_context(InvalidExportName)?;
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
    arg: &ShortCStr,
    eq_pos: usize,
    state: &mut ShellState,
) -> Result<(), ShortCStrError> {
    let bytes = arg.as_bytes()?;
    let name_bytes = bytes
        .get(..eq_pos)
        .ok_or(ShortCStrError::BadState)?
        .to_vec();
    let val_bytes = bytes
        .get(eq_pos + 1..)
        .ok_or(ShortCStrError::BadState)?
        .to_vec();
    let name_cstr = ShortCStr::from_vec(name_bytes)?;
    state
        .strings
        .insert(name_cstr.clone(), ShortCStr::from_vec(val_bytes.clone())?);
    state.exports.insert(name_cstr, val_bytes);
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
