// mutants: -C --lib -C --test -C repl -- -E 'test(/^repl::/) | binary(repl)'

//! `PS0` — the value bash ≥ 5.1 **displays** after reading a complete input line
//! and before its command runs. bash's `NEWS` entry says "executed", and the
//! measured behavior, `man bash` and bash's own `eval.c` (`decode_prompt_string` +
//! `fprintf(stderr, …)`) all say **displayed**: `PS0='echo FROMPS0'` prints the
//! text `echo FROMPS0`, and a `PS0='echo x >> file'` never creates the file. So
//! this drawer expands and writes; nothing is executed, and `$?` is trivially
//! preserved. Unset or set-empty (the default) answers nothing, so the whole
//! feature is a no-op until someone sets it.
//!
//! It is the third drawer of the pipeline in `prompt.rs`. Three policies are its
//! own, all measured against bash 5.3.9 on 2026-11-20: the tty gate is
//! `PS1`/`PS2`'s — nothing off a terminal — while the `promptvars` shopt gates its
//! expansion exactly as it gates theirs and `PS4`; the marker policy is
//! `PS1`/`PS2`'s `Markers::Bytes`, so a stray `\[…\]` escape keeps readline's byte
//! exactly as bash's display does (`a\]b` → `a\002b`), while a **toggling** pair is
//! consumed as a record split and contributes no bytes — where bash's raw display
//! prints its byte, the divergence #623 words; and there is **no default string** —
//! unset and set-empty both answer nothing. The drawer is **stderr** with **no
//! trailing newline**, and its display runs **before** the command counter moves,
//! which is the number it shows (bash's own comment in `reader_loop` puts it before
//! the increment). Blank lines are not commands, so they get neither a display nor a
//! count.

use alloc::vec::Vec;
use error_stack::{Report, ResultExt};

use super::decode::Markers;
use super::render::PromptVars;
use super::resolve_bytes;

use crate::app::AppError;
use crate::options::PROMPTVARS;
use crate::state::ShellState;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

/// `PS0`'s bytes for one display, through the shared pipeline under the three
/// rules named in the module doc. The two policies are read in a block so no
/// borrow of the state is held across the expansion (LESSONS: "Never hold a
/// ForkCell borrow across a recursive call that re-borrows"); the empty `default`
/// is the whole point — there is no fallback string to fall back to.
fn resolve(cell: &ForkCell<ShellState>) -> Result<Vec<u8>, Report<AppError>> {
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
    resolve_bytes(cell, &ShortCStr::from(c"PS0"), b"", policy, Markers::Bytes)
}

/// Display `PS0` for the input just read, before its command runs. A diagnostic
/// path cannot fail: a borrow conflict or a failed expansion answers nothing, and
/// the write's own error is ignored — the `xtrace.rs::emit` precedent (STYLE
/// §4.4). Nothing is written for an empty answer, and no newline is appended.
pub(crate) fn display(cell: &ForkCell<ShellState>) {
    let Ok(bytes) = resolve(cell) else {
        return;
    };
    if !bytes.is_empty() {
        let _ = sys::ERR.write_all(&bytes);
    }
}

#[cfg(test)]
mod tests;
