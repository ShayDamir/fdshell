//! The `help` command's two lists, kept in sync with the dispatch tables:
//!
//! - `SHELL_CMDS` (`shell.rs`): commands the shell handles in-process (the
//!   `intercept` table in `intercept.rs` plus the parse-level
//!   `umask`/`unset`).
//! - `BUILTINS` (`builtins.rs`): the `DISPATCH` table in
//!   `child/dispatch.rs` plus the `import_fd`/`export_fd` fd-pass builtins
//!   in `child/fdpass.rs`.
//!
//! The unit tests in `help/tests.rs` and `tests/help.rs` assert this holds.

mod builtins;
mod shell;

pub(crate) use builtins::BUILTINS;
pub(crate) use shell::SHELL_CMDS;
