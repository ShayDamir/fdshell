//! REPL continuation check for a `<<` run: its delimiter line is still
//! missing, so more input could complete it.

use super::full::body_regions_full;
use super::line::line_end_after;
use super::operator_delims;

/// `true` when `line[run_start..run_end]` carries a `<<` operator whose
/// delimiter line has not arrived yet, so the REPL should keep reading.
///
/// The body is read after the run's whole logical line, so a `;`/`&&`/`||`
/// after the operator does not end the search: the delimiter line is looked
/// for after the line's newline (or after the buffer end when the line is
/// not finished). `false` for a run whose delimiter line is already present
/// and a bare `<<` with no delimiter word (no further input can fix it).
pub(crate) fn unterminated(line: &[u8], run_start: usize, run_end: usize) -> bool {
    let ops = match operator_delims(line, run_start, run_end) {
        Some(ops) if !ops.is_empty() => ops,
        _ => return false,
    };
    let line_end = line_end_after(line, run_start);
    body_regions_full(line, line_end, &ops).is_err()
}
