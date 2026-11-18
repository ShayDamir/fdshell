// mutants: -C --lib -C --test -C repl -- -E 'test(/^repl::/) | binary(repl)'

//! Resolving the prompt bytes to draw before one read: the `PS1`/`PS2` shell
//! variable when stdin is a terminal, the bash default when the variable is
//! unset, and nothing off-terminal (bash draws no prompt there either). The
//! bytes are then decoded and rendered — the record and the contract of that
//! pass are in the module doc of `decode.rs`.

mod decode;
mod render;
mod segment;
mod value;

use decode::decode;
use render::render;
use value::NoValues;

use alloc::vec::Vec;
use error_stack::{Report, ResultExt};

use crate::app::AppError;
use crate::state::ShellState;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

/// The bytes to draw before one read: `name`'s value when stdin is a terminal
/// — a set-empty value draws nothing — the bash `default` when the variable is
/// unset (which is why the default is a read-time fallback and not a seeded
/// `strings` row: seeding would erase the unset-vs-empty distinction), and
/// nothing when it is not. The bytes are then decoded into the record
/// (`decode.rs`) and rendered through the no-values provider — value escapes
/// render as their literal text until #617–#619 wire real providers. bash
/// re-reads the variable for every prompt, so an assignment run as a command
/// changes the *next* prompt. The bytes are owned and the borrow ends inside
/// the block, before the caller's `write_all` syscall (LESSONS: never hold a
/// `Ref` across a syscall).
pub(crate) fn resolve(
    cell: &ForkCell<ShellState>,
    name: &ShortCStr,
    default: &[u8],
) -> Result<Vec<u8>, Report<AppError>> {
    let drawn = {
        let state = cell.borrow().change_context(AppError::Borrow)?;
        if !state.interactive {
            return Ok(Vec::new());
        }
        match state.var_value(name) {
            None => default.to_vec(),
            Some(value) => value.as_bytes().change_context(AppError::Never)?.to_vec(),
        }
    };
    Ok(render(&decode(&drawn), &NoValues))
}

#[cfg(test)]
mod tests;
