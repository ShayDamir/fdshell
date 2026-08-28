use builtins::error::BuiltinError;
use error_stack::{Report, ResultExt};

use self::commands::{BUILTINS, SHELL_CMDS};

mod commands;

pub(crate) fn print_help() -> Result<i32, Report<BuiltinError>> {
    sys::OUT
        .write_all(b"Shell commands:\n\n")
        .change_context(BuiltinError::Io)?;
    print_list(SHELL_CMDS)?;
    sys::OUT
        .write_all(b"\nBuiltins:\n\n")
        .change_context(BuiltinError::Io)?;
    print_list(BUILTINS)?;
    Ok(0)
}

fn print_list(entries: &[(&[u8], &[u8])]) -> Result<(), Report<BuiltinError>> {
    let max_name = entries
        .iter()
        .map(|(name, _)| name.len())
        .max()
        .unwrap_or(0);
    for (name, desc) in entries {
        sys::OUT.write_all(name).change_context(BuiltinError::Io)?;
        for _ in name.len()..max_name {
            sys::OUT.write_all(b" ").change_context(BuiltinError::Io)?;
        }
        sys::OUT.write_all(b"  ").change_context(BuiltinError::Io)?;
        sys::OUT.write_all(desc).change_context(BuiltinError::Io)?;
        sys::OUT.write_all(b"\n").change_context(BuiltinError::Io)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
