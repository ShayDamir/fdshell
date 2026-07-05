use builtins::error::BuiltinError;
use error_stack::Report;
use std::io::Write;

const SHELL_CMDS: &[(&[u8], &[u8])] = &[
    (b"become", b"Replace shell with command"),
    (b"cd", b"Change directory"),
    (b"envfilter", b"Filter env vars for child processes"),
    (b"exit", b"Exit shell"),
    (b"export", b"Set or list exports"),
    (b"export_fd", b"Export fd to variable"),
    (b"shift", b"Shift positional parameters"),
    (b"umask", b"Set or show file mode mask"),
    (b"unset", b"Remove variable"),
    (b"wait", b"Wait for background tasks"),
];

const BUILTINS: &[(&[u8], &[u8])] = &[
    (b"echo", b"Print arguments"),
    (b"exec_at", b"Execute with path lookup"),
    (b"exec_fd", b"Execute with fd lookup"),
    (b"false", b"Exit with failure status"),
    (b"fchmod", b"Change file mode"),
    (b"help", b"List available commands"),
    (b"mkdirat", b"Create directory"),
    (b"openat2", b"Open file"),
    (b"pipe", b"Create pipe"),
    (b"pwd", b"Print working directory"),
    (b"renameat2", b"Rename/move file"),
    (b"resolve", b"Resolve fd variables"),
    (b"true", b"Exit with success status"),
];

pub(crate) fn print_help() -> Result<i32, Report<BuiltinError>> {
    use error_stack::ResultExt;
    let mut lock = std::io::stdout().lock();
    lock.write_all(b"Shell commands:\n\n")
        .change_context(BuiltinError::Io)?;
    print_list(&mut lock, SHELL_CMDS)?;
    lock.write_all(b"\nBuiltins:\n\n")
        .change_context(BuiltinError::Io)?;
    print_list(&mut lock, BUILTINS)?;
    Ok(0)
}

fn print_list(
    lock: &mut impl Write,
    entries: &[(&[u8], &[u8])],
) -> Result<(), Report<BuiltinError>> {
    use error_stack::ResultExt;
    let max_name = entries
        .iter()
        .map(|(name, _)| name.len())
        .max()
        .unwrap_or(0);
    for (name, desc) in entries {
        lock.write_all(name).change_context(BuiltinError::Io)?;
        for _ in name.len()..max_name {
            lock.write_all(b" ").change_context(BuiltinError::Io)?;
        }
        lock.write_all(b"  ").change_context(BuiltinError::Io)?;
        lock.write_all(desc).change_context(BuiltinError::Io)?;
        lock.write_all(b"\n").change_context(BuiltinError::Io)?;
    }
    Ok(())
}
