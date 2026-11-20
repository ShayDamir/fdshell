// mutants: -C --lib -C --test -C xtrace -- -E 'test(/^xtrace::/) | binary(xtrace)'

//! xtrace (`set -x` / `set +x`): print `<prefix><name> <args>` to stderr at each
//! dispatch entry, before the command runs.
//!
//! The prefix is the `PS4` variable, decoded and expanded **per use**: bash
//! re-expands it at every trace line, so `emit` asks `repl/prompt.rs`'s
//! `resolve_ps4` once per line (one fork per `$( … )`, as bash pays). The default
//! `+ ` is a read-time fallback — bash *seeds* `PS4` as a real variable, so an
//! unset one draws `+ ` here and a set-empty one draws nothing (measured). A
//! non-toggling `\[`…`\]` marker escape contributes nothing here: stderr is a raw
//! drawer. `${BASH_SOURCE}`, `${BASH_LINENO}` and `${BASH_COMMAND}` are ordinary
//! variables for fdshell — bash maintains their values per trace line, the
//! divergence the README records for task #621.
//!
//! Child dispatch prints substituted (expanded) arguments; intercepts print the
//! raw arguments.
//!
//! Security note: expanded values can carry secrets into stderr.

use alloc::vec::Vec;

use crate::options::XTRACE;
use crate::repl::prompt::resolve_ps4;
use crate::state::ShellState;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

/// The measured bash default, kept as a read-time fallback: an unset `PS4` draws
/// it, a set-empty `PS4` draws nothing (which seeding the table would erase).
const DEFAULT_PS4: &[u8] = b"+ ";

/// xtrace a command from a `ForkCell` (the cell, never a borrow held here: the
/// prefix re-borrows it and can fork — see `emit`).
pub(crate) fn trace_cmd(
    name: &[u8],
    cmdline: &crate::parse::CommandLine,
    cell: &ForkCell<ShellState>,
) {
    if traced(cell) {
        emit(name, &cmdline.args, cell);
    }
}

pub(crate) fn trace(name: &[u8], args: &[ShortCStr], cell: &ForkCell<ShellState>) {
    if traced(cell) {
        emit(name, args, cell);
    }
}

/// Trace a command regardless of the xtrace option — for `set -x`/`set +x`
/// themselves, which print in both directions, as in bash.
pub(crate) fn trace_unconditional(name: &[u8], args: &[ShortCStr], cell: &ForkCell<ShellState>) {
    emit(name, args, cell);
}

/// The option, read through a borrow that ends here; a conflict draws no line,
/// which is what `trace_cmd`'s skipped borrow always did.
fn traced(cell: &ForkCell<ShellState>) -> bool {
    cell.borrow().is_ok_and(|state| state.option_on(XTRACE))
}

/// One trace line: the prefix resolved first — it re-borrows the cell and can
/// fork for a `$( … )` — then the write with no borrow held (LESSONS: never hold
/// a `Ref` across a syscall). A diagnostic path cannot fail, so a borrow conflict
/// or a resolve error falls back to the default prefix and stays silent; the
/// write's own error was already ignored.
fn emit(name: &[u8], args: &[ShortCStr], cell: &ForkCell<ShellState>) {
    let prefix = resolve_ps4(cell, DEFAULT_PS4).unwrap_or_else(|_| DEFAULT_PS4.to_vec());
    let _ = sys::ERR.write_all(&format_trace(name, args, &prefix));
}

fn format_trace(name: &[u8], args: &[ShortCStr], prefix: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(prefix);
    out.extend_from_slice(name);
    for arg in args {
        out.push(b' ');
        out.extend_from_slice(arg.as_bytes().unwrap_or(&[]));
    }
    out.push(b'\n');
    out
}
