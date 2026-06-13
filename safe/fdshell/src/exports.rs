use crate::state::ShellState;
use sys::ShortCStr;

pub fn handle_export(args: &[ShortCStr], state: &mut ShellState) -> Result<(), i32> {
    match args.first() {
        None => list_exports(state),
        Some(arg) => {
            if let Some(eq_pos) = arg.as_bytes()?.iter().position(|&b| b == b'=') {
                set_export(arg, eq_pos, state)?;
            } else {
                mark_exported(arg, state)?;
            }
            Ok(())
        }
    }
}

fn list_exports(state: &ShellState) -> Result<(), i32> {
    for (k, v) in &state.exports {
        let key_str = core::str::from_utf8(k.as_bytes().unwrap_or(&[])).unwrap_or("?");
        let val_str = core::str::from_utf8(v.as_slice()).unwrap_or("?");
        println!("export {}={}", key_str, val_str);
    }
    Ok(())
}

fn set_export(arg: &ShortCStr, eq_pos: usize, state: &mut ShellState) -> Result<(), i32> {
    let bytes = arg.as_bytes()?;
    let name_bytes = bytes.get(..eq_pos).ok_or(sys::errno::EINVAL)?.to_vec();
    let val_bytes = bytes.get(eq_pos + 1..).ok_or(sys::errno::EINVAL)?.to_vec();
    let name_cstr = ShortCStr::from_vec(name_bytes)?;
    state
        .strings
        .insert(name_cstr.clone(), ShortCStr::from_vec(val_bytes.clone())?);
    state.exports.insert(name_cstr, val_bytes);
    Ok(())
}

fn mark_exported(arg: &ShortCStr, state: &mut ShellState) -> Result<(), i32> {
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
