//! REPL continuation check for a `<<` run: its delimiter line is still
//! missing, so more input could complete it.

use super::delims::operator_delims;
use super::skip_region;

/// `true` when `line[run_start..run_end]` carries a `<<` operator whose
/// delimiter line has not arrived yet, so the REPL should keep reading.
///
/// `false` for a `;`-terminated run (the parser rejects it outright), a run
/// whose delimiter line is already present, and a bare `<<` with no delimiter
/// word (no further input can fix it).
pub(crate) fn unterminated(line: &[u8], run_start: usize, run_end: usize) -> bool {
    if !matches!(line.get(run_end), Some(&b'\n') | None) {
        return false;
    }
    if skip_region(line, run_start, run_end).is_some() {
        return false;
    }
    operator_delims(line, run_start, run_end).is_some_and(|d| !d.is_empty())
}
