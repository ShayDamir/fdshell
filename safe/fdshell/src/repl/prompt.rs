// mutants: -C --lib -C --test -C repl -- -E 'test(/^repl::/) | binary(repl)'

//! Resolving the prompt bytes to draw before one read: the `PS1`/`PS2` shell
//! variable when stdin is a terminal, the bash default when the variable is
//! unset, and nothing off-terminal (bash draws no prompt there either). The
//! bytes are then decoded and rendered — the record and the contract of that
//! pass are in the module doc of `decode.rs`. Between the two sits the
//! expansion stage (task #620): literal runs are expanded by parameter
//! expansion, command substitution and arithmetic expansion, gated by the
//! `promptvars` shopt and read here so no borrow of the state is held across it
//! (LESSONS: "Never hold a ForkCell borrow across a recursive call that
//! re-borrows"); the grammar of that stage is in `render/expand.rs`. `PS4` (task
//! #621) shares the pipeline through `resolve_ps4`: the shopt gates it as it
//! gates these two, while the tty gate and the marker bytes are the drawers' own.

mod decode;
mod render;
mod segment;
mod value;

use decode::{Markers, decode};
use render::{PromptVars, render};
use value::clock::ClockValues;
use value::composite::Providers;
use value::cwd::CwdValues;
use value::shell::ShellValues;

use alloc::vec::Vec;
use error_stack::{Report, ResultExt};

use crate::app::AppError;
use crate::options::PROMPTVARS;
use crate::state::ShellState;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

/// The pipeline both drawers share: read `name`'s bytes — the bash `default` when
/// the variable is unset (which is why the default is a read-time fallback and not
/// a seeded `strings` row: seeding would erase the unset-vs-empty distinction),
/// its value otherwise, a set-empty value answering nothing — then decode and
/// render them through the composite of the value providers (`value.rs`'s seam):
/// every family now answers real values — `\u \h \H \l \$ \s \v \V \j \! \#`
/// (task #617), `\d \t \T \@ \A` (task #618) and `\w \W` from the `%CWD` fd's
/// provenance (task #619) — and an escape whose provider has no value for it
/// renders as its literal text (decision D2). The gate, the `promptvars` policy
/// and the marker policy are the caller's, read and decided before the expansion
/// stage so no borrow of the state is held across it (LESSONS: "Never hold a
/// ForkCell borrow across a recursive call that re-borrows"); the bytes are owned
/// and the borrow ends inside the block, before the caller's `write_all` syscall
/// (LESSONS: never hold a `Ref` across a syscall).
fn resolve_bytes(
    cell: &ForkCell<ShellState>,
    name: &ShortCStr,
    default: &[u8],
    policy: PromptVars,
    markers: Markers,
) -> Result<Vec<u8>, Report<AppError>> {
    let drawn = {
        let state = cell.borrow().change_context(AppError::Borrow)?;
        match state.var_value(name) {
            None => default.to_vec(),
            Some(value) => value.as_bytes().change_context(AppError::Never)?.to_vec(),
        }
    };
    let shell = ShellValues::new(cell);
    let clock = ClockValues::new();
    let cwd = CwdValues::new(cell);
    let record = render(
        &decode(&drawn, markers),
        &Providers::new(&shell, &clock, &cwd),
        policy,
        cell,
    );
    Ok(record
        .iter()
        .flat_map(|run| run.bytes.iter())
        .copied()
        .collect())
}

/// The bytes to draw before one read: `PS1`/`PS2`'s value when stdin is a
/// terminal, and nothing off-terminal (bash draws no prompt there either). Both
/// go through the `promptvars` shopt, on by default. bash re-reads the variable
/// for every prompt, so an assignment run as a command changes the *next* prompt.
pub(crate) fn resolve(
    cell: &ForkCell<ShellState>,
    name: &ShortCStr,
    default: &[u8],
) -> Result<Vec<u8>, Report<AppError>> {
    let policy = {
        let state = cell.borrow().change_context(AppError::Borrow)?;
        if !state.interactive {
            return Ok(Vec::new());
        }
        if state.option_on(PROMPTVARS) {
            PromptVars::On
        } else {
            PromptVars::Off
        }
    };
    resolve_bytes(cell, name, default, policy, Markers::Bytes)
}

/// `PS4`'s bytes for one trace line (task #621), the same pipeline under the two
/// rules bash itself applies to `PS4` and not to `PS1`/`PS2`: it is expanded at
/// **every use** — one fork per `$( … )`, as in bash — and the tty gate does not
/// apply, because `PS4` works off-terminal (measured in `bash -c`) where
/// `PS1`/`PS2` draw nothing. The `promptvars` shopt **does** gate it, as it gates
/// the two prompt drawers (measured: `shopt -u promptvars` leaves `PS4`
/// unexpanded too), and a non-toggling `\[…\]` marker escape contributes nothing,
/// because stderr is a raw drawer (`Markers::Dropped`) while the prompt draws
/// readline's byte.
pub(crate) fn resolve_ps4(
    cell: &ForkCell<ShellState>,
    default: &[u8],
) -> Result<Vec<u8>, Report<AppError>> {
    let policy = {
        let state = cell.borrow().change_context(AppError::Borrow)?;
        if state.option_on(PROMPTVARS) {
            PromptVars::On
        } else {
            PromptVars::Off
        }
    };
    resolve_bytes(cell, &ShortCStr::from(c"PS4"), default, policy, Markers::Dropped)
}

#[cfg(test)]
mod tests;
