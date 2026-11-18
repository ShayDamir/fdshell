// mutants: -C --lib -C --test -C repl -- -E 'test(/^repl::/) | binary(repl)'

//! Rendering the decoded record into the runs to draw: each literal run through
//! the expansion stage (`render/expand.rs`), each `Value` placeholder through
//! the provider, and the concatenation reproduces bash's measured byte stream.
//!
//! The returned record is the one epic #198 must read (D-620-1): kinds
//! preserved, bytes final. With `promptvars` on, the drawn bytes are the
//! expanded ones, so a record still holding the unexpanded bytes would answer
//! the width question wrongly; `prompt.rs::resolve` concatenates this record to
//! get the bytes, and #198 reads it to know which of them count.
//!
//! The two paste rules, both measured against bash 5.3.9 on 2026-11-18:
//!
//! - a `Value` placeholder is pasted **verbatim**, the provider's answer and the
//!   decision D2 fallback alike (D-620-2): bytes a provider produced are data,
//!   never triggers — that is what reproduces bash's `\$v` row (`\$` is the
//!   decoder's escape, its byte is not a trigger) and what protects the pins
//!   whose answers contain `~` or `$`;
//! - a `Visible` / `NonPrinting` run is expanded when the policy is `On`, and
//!   pasted as it stands when it is `Off` — decoding stays, the expansion is
//!   skipped, and quote removal is part of the skipped expansion.
//!
//! A failing expansion is reported and the run's literal bytes are pasted
//! (D-620-8): nounset, an unclosed `$(`, an unclosed backtick, an arithmetic
//! error and an over-cap capture are all reachable from a prompt, and bash
//! reports on stderr, draws the prompt as the unexpanded text, and keeps
//! prompting — the REPL must not die because a prompt grew past
//! `set --stdout-capture-limit` (D-620-10). The report is printed with the
//! REPL's own wording; bash's wording differs and #623 documents it.

mod expand;

use alloc::vec::Vec;
use core::fmt::Write;

use crate::io;
use crate::repl::prompt::render::expand::expand_run;
use crate::repl::prompt::segment::{Segment, SegmentKind};
use crate::repl::prompt::value::Values;
use crate::state::ShellState;
use sys::fork_cell::ForkCell;

/// Whether the expansion stage runs — the `promptvars` shopt as data, the
/// sibling of `substitute/surface.rs`'s policies (task #371's pattern): read
/// once by `resolve`, before any borrow that must not be held across the
/// expansion, and handed here.
pub(crate) enum PromptVars {
    /// bash's default: every literal run is expanded.
    On,
    /// `shopt -u promptvars`: every literal run is pasted as it stands.
    Off,
}

/// Render the record: provider answers and D2 fallbacks verbatim, literal runs
/// through the expansion stage when the policy says so, and every failing
/// expansion reported to stderr with its run's own bytes pasted.
pub(crate) fn render(
    record: &[Segment],
    values: &dyn Values,
    policy: PromptVars,
    cell: &ForkCell<ShellState>,
) -> Vec<Segment> {
    let mut out = Vec::new();
    for segment in record {
        let bytes = match &segment.kind {
            SegmentKind::Value(esc) => values.value(*esc).unwrap_or_else(|| segment.bytes.clone()),
            SegmentKind::Visible | SegmentKind::NonPrinting => match policy {
                PromptVars::Off => segment.bytes.clone(),
                PromptVars::On => match expand_run(&segment.bytes, cell) {
                    Ok(bytes) => bytes,
                    Err(report) => {
                        let _ = writeln!(io::Stderr, "{report:?}");
                        segment.bytes.clone()
                    }
                },
            },
        };
        out.push(Segment {
            kind: segment.kind,
            bytes,
        });
    }
    out
}

#[cfg(test)]
mod tests;
