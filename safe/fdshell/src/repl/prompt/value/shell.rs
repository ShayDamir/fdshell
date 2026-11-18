// mutants: -C --lib -C --test -C repl -- -E 'test(/^repl::/) | binary(repl)'

//! Task #617's provider: the values of `\u \h \H \l \$ \s \v \V \j \! \#`, asked
//! of `ShellState` and the `sys` wrappers one escape at a time, at render time —
//! a prompt that names no value escape pays no syscall, and no snapshot is
//! plumbed through the seam (decision D3's lazy shape).
//!
//! Each escape has exactly one data source — a syscall **or** a state read,
//! never both — so no borrow is ever held across a syscall (LESSONS). Nothing is
//! cached: bash caches the username and hostname at startup and `ttyname` per fd
//! and re-runs `geteuid` per expansion; a prompt is interactive-only work, so
//! fdshell just asks, and a hostname change shows on the next prompt.
//!
//! A source that fails answers `None` — decision D2's "no value", which renders
//! the placeholder's literal escape text. That is fdshell's own definition, and
//! the two places where bash answers something else are divergences for README
//! task #623 (measured against bash 5.3.9 on 2026-11-18): no terminal, where bash
//! prints `tty`, and — for `\s \v \V` — bash's own identity (`bash-5.3$ `), where
//! fdshell draws its program name and its version.

use core::fmt::Display;

use alloc::format;
use alloc::vec::Vec;

use crate::repl::prompt::segment::ValueEscape;
use crate::repl::prompt::value::Values;
use crate::state::ShellState;
use sys::IN;
use sys::fork_cell::ForkCell;
use sys::getpwuid::getpwuid;
use sys::unistd::{geteuid, gethostname};

/// The provider over the shell state's cell: the REPL's `resolve` renders through
/// it, so every value escape below answers a real value.
pub(crate) struct ShellValues<'a> {
    cell: &'a ForkCell<ShellState>,
}

impl<'a> ShellValues<'a> {
    pub(crate) fn new(cell: &'a ForkCell<ShellState>) -> Self {
        Self { cell }
    }

    /// The state-sourced answers: one borrow, released before the answer leaves.
    /// A failed borrow is a source that failed, so it answers `None` like the
    /// rest (the trait's signature carries no error channel).
    fn answer_from_state(
        &self,
        answer: impl FnOnce(&ShellState) -> Option<Vec<u8>>,
    ) -> Option<Vec<u8>> {
        let state = self.cell.borrow().ok()?;
        answer(&state)
    }
}

impl Values for ShellValues<'_> {
    fn value(&self, esc: ValueEscape) -> Option<Vec<u8>> {
        match esc {
            ValueEscape::Username => getpwuid(geteuid()),
            ValueEscape::Hostname => gethostname().ok().map(|name| cut_at_first_dot(&name)),
            ValueEscape::FullHostname => gethostname().ok(),
            ValueEscape::TtyName => IN.ttyname().ok().map(|path| basename(&path)),
            ValueEscape::Dollars => Some(dollars()),
            ValueEscape::ShellName => self.answer_from_state(|state| {
                state.program_name.as_bytes().ok().map(|name| name.to_vec())
            }),
            ValueEscape::Version | ValueEscape::PlusVersion => Some(version()),
            ValueEscape::Jobs => self.answer_from_state(|state| {
                Some(decimal(state.jobs.named_len() + state.jobs.anon_len()))
            }),
            ValueEscape::History | ValueEscape::AlignedNumber => {
                self.answer_from_state(|state| Some(decimal(state.command_number + 1)))
            }
            // #618's clock escapes and #619's working directory: no values yet,
            // so the renderer's D2 fallback keeps drawing them literally.
            _ => None,
        }
    }
}

/// `\v` and `\V`: fdshell has no patchlevel concept, so both draw the crate's own
/// version (the epic's minimal definition).
fn version() -> Vec<u8> {
    env!("CARGO_PKG_VERSION").as_bytes().to_vec()
}

/// `\u`'s companion `\$`: `#` for the superuser, `$` for everyone else — the
/// **effective** uid, which bash re-reads per expansion (no cache).
fn dollars() -> Vec<u8> {
    if geteuid() == 0 {
        b"#".to_vec()
    } else {
        b"$".to_vec()
    }
}

/// `\h`: the hostname up to the first dot, the whole name when it has none —
/// measured on a dotless hostname, where bash's `\h` and `\H` agree.
fn cut_at_first_dot(name: &[u8]) -> Vec<u8> {
    name.split(|byte| *byte == b'.')
        .next()
        .unwrap_or_default()
        .to_vec()
}

/// `\l`: the tty's last path component — the pts number here, which is what bash
/// draws too (measured `[3]` for `/dev/pts/3`, 2026-11-18), so no divergence.
fn basename(path: &[u8]) -> Vec<u8> {
    path.rsplit(|byte| *byte == b'/')
        .next()
        .unwrap_or_default()
        .to_vec()
}

/// A count as decimal bytes — the shape `\j` and the two counters draw.
fn decimal<T: Display>(value: T) -> Vec<u8> {
    format!("{value}").into_bytes()
}

#[cfg(test)]
mod tests;
