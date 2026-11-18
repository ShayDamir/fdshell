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
//! re-borrows"); the grammar of that stage is in `render/expand.rs`.

mod decode;
mod render;
mod segment;
mod value;

use decode::decode;
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

/// The bytes to draw before one read: `name`'s value when stdin is a terminal
/// — a set-empty value draws nothing — the bash `default` when the variable is
/// unset (which is why the default is a read-time fallback and not a seeded
/// `strings` row: seeding would erase the unset-vs-empty distinction), and
/// nothing when it is not. The bytes are then decoded into the record
/// (`decode.rs`) and rendered through the composite of the value providers
/// (`value.rs`'s seam): every family now answers real values — `\u \h \H \l
/// \$ \s \v \V \j \! \#` (task #617), `\d \t \T \@ \A` (task #618) and `\w \W`
/// from the `%CWD` fd's provenance (task #619) — and an escape whose provider
/// has no value for it renders as its literal text (decision D2). bash
/// re-reads the variable for every prompt, so an assignment run as a command
/// changes the *next* prompt. The bytes are owned and the borrow ends inside
/// the block, before the caller's `write_all` syscall (LESSONS: never hold a
/// `Ref` across a syscall).
pub(crate) fn resolve(
    cell: &ForkCell<ShellState>,
    name: &ShortCStr,
    default: &[u8],
) -> Result<Vec<u8>, Report<AppError>> {
    let (drawn, policy) = {
        let state = cell.borrow().change_context(AppError::Borrow)?;
        if !state.interactive {
            return Ok(Vec::new());
        }
        let policy = if state.option_on(PROMPTVARS) {
            PromptVars::On
        } else {
            PromptVars::Off
        };
        let drawn = match state.var_value(name) {
            None => default.to_vec(),
            Some(value) => value.as_bytes().change_context(AppError::Never)?.to_vec(),
        };
        (drawn, policy)
    };
    let shell = ShellValues::new(cell);
    let clock = ClockValues::new();
    let cwd = CwdValues::new(cell);
    let record = render(
        &decode(&drawn),
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

#[cfg(test)]
mod tests;
