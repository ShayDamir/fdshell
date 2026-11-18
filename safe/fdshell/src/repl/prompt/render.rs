// mutants: -C --lib -C --test -C repl -- -E 'test(/^repl::/) | binary(repl)'

//! Rendering the decoded record into the bytes to draw: concatenate every
//! run, asking the provider to substitute each `Value` placeholder. The
//! concatenation reproduces bash's measured byte stream; epic #198 keeps the
//! record itself to know which runs count toward the width.

use alloc::vec::Vec;

use crate::repl::prompt::segment::{Segment, SegmentKind};
use crate::repl::prompt::value::Values;

/// Concatenate the record: literal runs as they are, `Value` placeholders
/// through the provider, falling back to the placeholder's own text.
pub(crate) fn render(record: &[Segment], values: &dyn Values) -> Vec<u8> {
    let mut out = Vec::new();
    for segment in record {
        match &segment.kind {
            SegmentKind::Value(esc) => {
                if let Some(value) = values.value(*esc) {
                    out.extend_from_slice(&value);
                    continue;
                }
                out.extend_from_slice(&segment.bytes);
            }
            SegmentKind::Visible | SegmentKind::NonPrinting => {
                out.extend_from_slice(&segment.bytes)
            }
        }
    }
    out
}

#[cfg(test)]
mod tests;
