//! The non-`builtin` branch of `replacer::execute`: the `builtin_first`
//! option dispatch, then external commands (hash-table aware).

use crate::child;
use crate::error::child_process::ChildProcessError;
use crate::exec;
use crate::mask::MaskBit;
use crate::state::ShellState;
use crate::substitute::substitute_args;
use alloc::vec::Vec;
use core::ffi::CStr;
use error_stack::{Report, ResultExt};
use sys::ShortCStr;
use sys::fork_cell::ForkCell;
use sys::signal;

pub(super) fn run(
    args: &[ShortCStr],
    args_mask: &[Vec<MaskBit>],
    args_span: &[Vec<usize>],
    args_quoted: &[bool],
    cell: &ForkCell<ShellState>,
) -> Result<i32, Report<ChildProcessError>> {
    // The `exec`/`become` command word is a word-0 position, so it takes the
    // word-0 expansion (task #268): `exec $(echo echo) hi` runs `echo`.
    // `fields[0]` is the binary and `fields[1..]` are its leading argv entries,
    // already expanded — argv is built here, so they never re-enter the
    // argument pipeline and need no quote encoding.
    let raw = args.first().ok_or(ChildProcessError::MissingArg)?;
    let mask = args_mask.first().map(Vec::as_slice).unwrap_or(&[]);
    let spans = args_span.first().map(Vec::as_slice).unwrap_or(&[]);
    let quoted = args_quoted.first().copied().unwrap_or(false);
    let fields = crate::command_word::fields(raw, mask, spans, quoted, cell)
        .change_context(ChildProcessError::ExecFailed)?;
    let binary = fields.first().cloned().unwrap_or_else(ShortCStr::new);
    let rest = args.get(1..).unwrap_or(&[]);
    let is_builtin = {
        let state = cell
            .borrow()
            .change_context(ChildProcessError::ExecFailed)?;
        child::dispatch::builtin_first(&binary, &state)
    };
    let mut words: Vec<ShortCStr> = fields.into_iter().skip(1).collect();
    words.extend(
        substitute_args(
            rest,
            args_mask.get(1..).unwrap_or(&[]),
            args_span.get(1..).unwrap_or(&[]),
            args_quoted.get(1..).unwrap_or(&[]),
            cell,
        )
        .change_context(ChildProcessError::ExecFailed)?,
    );
    let sealed: Vec<sys::ExportedCStr> = words.iter().map(|cs| cs.export()).collect();
    let refs: Vec<&CStr> = sealed.iter().map(|rc| rc.as_ref()).collect();
    if is_builtin {
        let state = cell
            .borrow()
            .change_context(ChildProcessError::ExecFailed)?;
        crate::xtrace::trace(binary.as_bytes().unwrap_or(&[]), &words, &state);
        return child::dispatch::run_builtin(binary.clone(), &refs, rest, &state);
    }
    // No caching here: `become`/`exec` exit the shell right after, so the
    // table would never survive — the lookup still honors existing pins.
    let fd = {
        let state = cell
            .borrow()
            .change_context(ChildProcessError::ExecFailed)?;
        exec::resolve_path(&binary, &state.hash_table, state.strict(), state.cwd_fd())
            .change_context(ChildProcessError::ExecFailed)?
    };
    let binary_exported = binary.export();
    let mut argv: Vec<&CStr> = alloc::vec![binary_exported.as_ref()];
    argv.extend_from_slice(&refs);
    let state = cell
        .borrow()
        .change_context(ChildProcessError::ExecFailed)?;
    crate::xtrace::trace(binary.as_bytes().unwrap_or(&[]), &words, &state);
    // `exec`/`become` replace *this* image, and this process is the shell:
    // Rust std's `SIG_IGN` for `SIGPIPE` is ours, not the program's. Clear it
    // so the program dies of the signal on a closed pipe, as in bash.
    // Best-effort: a failing sigaction leaves the inherited disposition.
    let _ = signal::reset_sigpipe();
    match exec::exec_fd(
        &fd,
        &argv,
        &state.environ,
        &state.exports,
        &state.env_filter,
        None,
    ) {
        Ok(()) => Ok(0),
        Err(report) => Err(report),
    }
}
