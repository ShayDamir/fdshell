// mutants: -C --lib -C --test -C repl -- -E 'test(/^repl::/) | binary(repl)'

//! Decoding the prompt bytes into the record epic #198's display-width math
//! consumes: an ordered `Vec<Segment>`, one run per kind, split exactly at
//! the toggling `\[`/`\]` markers, so the record alone answers which bytes
//! are visible.
//!
//! - `Visible` — bytes that count toward the display width: literal text,
//!   decoded mechanical/octal/hex bytes, and the bytes of non-toggling
//!   markers.
//! - `NonPrinting` — the bytes between a matched `\[`…`\]` pair: drawn, but
//!   excluded from the width sum by epic #198.
//! - `Value(escape)` — a value-escape placeholder; its `bytes` hold the raw
//!   escape text and the provider substitutes the real value at render time.
//!
//! The measured grammar (bash 5.3.9, measured 2026-11-18): the mechanical
//! escapes `\a \e \n \r \\`; octal is *exactly three* digits (unlike
//! `child/printf/escapes.rs` — LESSONS); `\x` with one or two hex digits is
//! an fdshell extension, decision D1, because bash measures it literal; a
//! `\[…\]` marker that does not toggle the region state is data, and its
//! byte is bash's marker byte (`\001`/`\002`, readline's
//! `RL_PROMPT_{START,END}_IGNORE`) — under the `Markers` policy handed at the
//! entry, which decides whether that byte reaches the record (`markers.rs`);
//! unknown escapes (including a trailing
//! `\`) keep the backslash and process the next byte as a literal — the same
//! convention `child/printf/escapes.rs` uses. Decode is infallible — every
//! input has a defined output — so the record comes straight back (STYLE
//! §4.12).

mod markers;
mod mech;

use alloc::{vec, vec::Vec};
use core::mem::take;

use crate::repl::prompt::segment::{Segment, SegmentKind, ValueEscape};
pub(crate) use markers::Markers;

/// readline's `RL_PROMPT_START_IGNORE` byte: a `\[` that does not open a
/// region is data, and bash emits this byte into the prompt (measured).
const MARKER_START: u8 = 0x01;

/// `RL_PROMPT_END_IGNORE`, the data byte of a `\]` that closes nothing.
const MARKER_END: u8 = 0x02;

/// What an escape resolves to: a byte into the current run (mechanical,
/// numeric, or a non-toggling marker), a run split at a toggling marker
/// (which is consumed), a value placeholder carrying its raw escape text, or
/// a marker the policy drops — consumed, contributing nothing.
enum Action {
    Byte(u8),
    Split(SegmentKind),
    Placeholder(ValueEscape, Vec<u8>),
    Skipped,
}

/// Decode the prompt bytes into the record the module doc defines, with the
/// marker policy `markers` (`markers.rs`).
pub(crate) fn decode(input: &[u8], markers: Markers) -> Vec<Segment> {
    let mut record = Vec::new();
    let mut run = Vec::new();
    let mut kind = SegmentKind::Visible;
    let mut i = 0;
    while let Some(&byte) = input.get(i) {
        if byte != b'\\' {
            run.push(byte);
            i += 1;
            continue;
        }
        let (action, consumed) = dispatch(input, i, kind, markers);
        match action {
            Action::Byte(b) => run.push(b),
            Action::Skipped => {}
            Action::Split(to) => {
                push_run(&mut record, &mut run, kind);
                kind = to;
            }
            Action::Placeholder(esc, bytes) => {
                push_run(&mut record, &mut run, kind);
                record.push(Segment {
                    kind: SegmentKind::Value(esc),
                    bytes,
                });
            }
        }
        i += consumed;
    }
    push_run(&mut record, &mut run, kind);
    record
}

/// Resolve the escape at `esc` (the backslash); `consumed` counts the bytes
/// it takes from the input — one for the keep-the-backslash rule, the escape
/// pair for a toggle or a single-letter escape, more for the numeric rules.
fn dispatch(input: &[u8], esc: usize, kind: SegmentKind, markers: Markers) -> (Action, usize) {
    let Some(&c) = input.get(esc + 1) else {
        return (Action::Byte(b'\\'), 1);
    };
    if let Some(action) = toggle(c, kind, markers) {
        return (action, 2);
    }
    if let Some(b) = mech::mechanical(c) {
        return (Action::Byte(b), 2);
    }
    if let Some(b) = mech::octal(input, esc + 1) {
        return (Action::Byte(b), 4);
    }
    if c == b'x' {
        if let Some((b, digits)) = mech::hex(input, esc + 2) {
            return (Action::Byte(b), 2 + digits);
        }
    } else if let Some(esc_kind) = mech::value_escape(c) {
        return (Action::Placeholder(esc_kind, vec![b'\\', c]), 2);
    }
    (Action::Byte(b'\\'), 1)
}

/// The `\[…\]` state machine: a marker that toggles the region state is
/// consumed and splits the record there; one that does not toggle is data, and
/// the `markers` policy says what its byte is worth (the constants, or nothing).
fn toggle(c: u8, kind: SegmentKind, markers: Markers) -> Option<Action> {
    match (c, kind) {
        (b'[', SegmentKind::Visible) => Some(Action::Split(SegmentKind::NonPrinting)),
        (b']', SegmentKind::NonPrinting) => Some(Action::Split(SegmentKind::Visible)),
        (b'[', SegmentKind::NonPrinting) => Some(markers.action(MARKER_START)),
        (b']', SegmentKind::Visible) => Some(markers.action(MARKER_END)),
        _ => None,
    }
}

/// Close the current run into the record unless it is empty; runs split only
/// where a toggling marker or a placeholder says so.
fn push_run(record: &mut Vec<Segment>, run: &mut Vec<u8>, kind: SegmentKind) {
    if run.is_empty() {
        return;
    }
    record.push(Segment {
        kind,
        bytes: take(run),
    });
}

#[cfg(test)]
mod tests;
