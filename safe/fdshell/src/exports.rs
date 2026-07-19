use crate::error::exports::ExportError;
use crate::state::ShellState;
use error_stack::{Report, ResultExt};
use sys::importedfd_io::ImportedFdIo;

use sys::ShortCStr;

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
                mark_exported(arg, state);
            }
            Ok(())
        }
    }
}

fn list_exports(state: &ShellState) -> Result<(), Report<ExportError>> {
    for (k, v) in &state.exports {
        let line = ShortCStr::concat(&[&c"export ".into(), k, &c"=".into(), v, &c"\n".into()])
            .change_context(ExportError::NulByte)?;
        sys::OUT.write_str(&line).change_context(ExportError::Io)?;
    }
    Ok(())
}

fn set_export(
    name: ShortCStr,
    value: ShortCStr,
    state: &mut ShellState,
) -> Result<(), Report<ExportError>> {
    state.exports.insert(name.clone(), value.clone());
    state.strings.insert(name, value);
    Ok(())
}

fn mark_exported(arg: &ShortCStr, state: &mut ShellState) {
    state.exports.insert(arg.clone(), ShortCStr::new());
}
