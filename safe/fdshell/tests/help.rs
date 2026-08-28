#![cfg_attr(test, allow(clippy::unwrap_used))]

use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_fdshell");

/// Commands the shell runs in-process: the `intercept` table plus the
/// parse-level `umask` / `unset`.
const SHELL_COMMANDS: &[&str] = &[
    "alias",
    "unalias",
    "become",
    "cd",
    "envfilter",
    "eval",
    "exec",
    "exit",
    "export",
    "export_fd",
    "hash",
    "read",
    "send_fd",
    "set",
    "shift",
    "shopt",
    "signalfd",
    "source",
    "timeout",
    "ulimit",
    "umask",
    "unset",
    "waitpid",
];

/// The `DISPATCH` builtins plus the `import_fd`/`export_fd` fd-pass builtins.
const BUILTINS: &[&str] = &[
    "[",
    "echo",
    "eventfd",
    "exec_at",
    "exec_fd",
    "explain",
    "false",
    "fchmod",
    "fdexplain",
    "fsync",
    "ftruncate",
    "help",
    "import_fd",
    "lseek",
    "mkdirat",
    "openat2",
    "pipe",
    "printf",
    "pwd",
    "renameat2",
    "resolve",
    "test",
    "timerfd",
    "true",
    "type",
];

/// The names listed by `help`, in its own words.
fn help_names() -> Vec<String> {
    let out = Command::new(BIN).args(["-c", "help"]).output().unwrap();
    assert!(out.status.success());
    let text = String::from_utf8(out.stdout).unwrap();
    text.lines()
        .filter_map(|line| line.split_whitespace().next())
        .filter(|tok| !matches!(*tok, "Shell" | "Builtins:"))
        .map(str::to_string)
        .collect()
}

#[test]
fn help_lists_every_shell_command() {
    let names = help_names();
    for name in SHELL_COMMANDS {
        assert!(
            names.iter().any(|n| n == name),
            "help is missing shell command `{name}`"
        );
    }
}

#[test]
fn help_lists_every_builtin() {
    let names = help_names();
    for name in BUILTINS {
        assert!(
            names.iter().any(|n| n == name),
            "help is missing builtin `{name}`"
        );
    }
}

/// No more, no fewer: the printed set must equal the supported set exactly.
#[test]
fn help_names_are_exactly_the_supported_commands() {
    let names = help_names();
    assert_eq!(
        names.len(),
        SHELL_COMMANDS.len() + BUILTINS.len(),
        "help printed: {names:?}"
    );
    for name in &names {
        assert!(
            SHELL_COMMANDS.contains(&name.as_str()) || BUILTINS.contains(&name.as_str()),
            "help lists `{name}` but it is not a supported command"
        );
    }
}
