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
            state.list_exports()?;
            Ok(())
        }
        Some(arg) => {
            if let Some((name, value)) = arg.split_once_byte(b'=') {
                state
                    .set_export(name, value, text)
                    .change_context(ExportError::NulByte)?;
            } else {
                state.mark_exported(arg);
            }
            Ok(())
        }
    }
}

impl ShellState {
    fn list_exports(&self) -> Result<(), Report<ExportError>> {
        for (k, v) in &self.exports {
            let line =
                ShortCStr::concat(&[&c"export ".into(), k, &c"=".into(), &v.value, &c"\n".into()]);
            sys::OUT.write_str(&line).change_context(ExportError::Io)?;
        }
        Ok(())
    }

    fn set_export(
        &mut self,
        name: ShortCStr,
        value: ShortCStr,
        text: &ScriptText,
    ) -> Result<(), Report<ExportError>> {
        let trace = Trace::at(text.start, text.origin.clone());
        let v = ImportedStr::new(value, trace);
        self.exports.insert(name.clone(), v.clone());
        self.set_var(name, v);
        Ok(())
    }

    /// Mark a bare `export NAME`: keep the existing traced value if the name is
    /// already set (shell string first, then inherited environment); otherwise
    /// export an empty shell value.
    fn mark_exported(&mut self, arg: &ShortCStr) {
        if let Some(existing) = self.strings.get(arg) {
            self.exports.insert(arg.clone(), existing.clone());
            return;
        }
        if let Some((_, v)) = self.environ.iter().find(|(k, _)| k == arg) {
            self.exports
                .insert(arg.clone(), ImportedStr::new(v.clone(), env_trace(arg)));
            return;
        }
        self.exports
            .insert(arg.clone(), ImportedStr::shell(ShortCStr::new()));
    }
}

fn env_trace(name: &ShortCStr) -> Trace {
    Trace::boundary(Origin::EnvVar(name.clone()))
}
