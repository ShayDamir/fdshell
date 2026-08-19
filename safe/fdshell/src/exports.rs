use crate::error::exports::ExportError;
use crate::state::ShellState;
use error_stack::{Report, ResultExt};

use sys::ShortCStr;
use sys::{ImportedStr, Origin, ScriptText, Trace};

pub fn handle_export(
    args: &[ShortCStr],
    text: &ScriptText,
    state: &mut ShellState,
) -> Result<(), Report<ExportError>> {
    match args.first() {
        None => {
            list_exports(state)?;
            Ok(())
        }
        Some(arg) => {
            if let Some((name, value)) = arg.split_once_byte(b'=') {
                set_export(name, value, text, state).change_context(ExportError::NulByte)?;
            } else {
                mark_exported(arg, state);
            }
            Ok(())
        }
    }
}

fn list_exports(state: &ShellState) -> Result<(), Report<ExportError>> {
    for (k, v) in &state.exports {
        let line =
            ShortCStr::concat(&[&c"export ".into(), k, &c"=".into(), &v.value, &c"\n".into()]);
        sys::OUT.write_str(&line).change_context(ExportError::Io)?;
    }
    Ok(())
}

fn set_export(
    name: ShortCStr,
    value: ShortCStr,
    text: &ScriptText,
    state: &mut ShellState,
) -> Result<(), Report<ExportError>> {
    let trace = Trace::at(text.start, text.origin.clone());
    let v = ImportedStr::new(value, trace);
    state.exports.insert(name.clone(), v.clone());
    state.strings.insert(name, v);
    Ok(())
}

/// Mark a bare `export NAME`: keep the existing traced value if the name is
/// already set (shell string first, then inherited environment); otherwise
/// export an empty shell value.
fn mark_exported(arg: &ShortCStr, state: &mut ShellState) {
    if let Some(existing) = state.strings.get(arg) {
        state.exports.insert(arg.clone(), existing.clone());
        return;
    }
    if let Some((_, v)) = state.environ.iter().find(|(k, _)| k == arg) {
        state
            .exports
            .insert(arg.clone(), ImportedStr::new(v.clone(), env_trace(arg)));
        return;
    }
    state
        .exports
        .insert(arg.clone(), ImportedStr::shell(ShortCStr::new()));
}

fn env_trace(name: &ShortCStr) -> Trace {
    Trace::boundary(Origin::EnvVar(name.clone()))
}
