//! Completeness check for a REPL input buffer: is it a complete construct,
//! or does it need more lines (continuation)?
//!
//! The check reuses the existing byte scanners verbatim (no parser changes):
//! `scan_segments` reports open keyword/function blocks and heredoc command
//! lines, and a `ScanState` walk reports open quotes/backticks/`$( )` and a
//! dangling top-level `&&`/`||`.

use crate::scan::heredoc;
use crate::segment::{Segment, scan_segments};

/// `true` when `line` is a complete construct and can be executed now;
/// `false` when it is incomplete (an open block, an unterminated heredoc, a
/// trailing operator, or an unbalanced quote/backtick/`$( )`) and the REPL
/// should read a continuation line.
pub(crate) fn is_complete(line: &[u8]) -> bool {
    !blocks_or_heredoc_open(line) && !super::trailing::trailing_open(line)
}

/// `true` when `line` carries an open keyword/function block or an
/// unterminated heredoc.
fn blocks_or_heredoc_open(line: &[u8]) -> bool {
    for seg in scan_segments(line, false) {
        match seg {
            Segment::Block { closed: false, .. } => return true,
            Segment::Statement { off, .. }
                if heredoc::unterminated(line, off, cmd_line_end(line, off)) =>
            {
                return true;
            }
            // A closed block (or any other segment) is complete.
            _ => {}
        }
    }
    false
}

/// The end of the command line starting at `off`: the first separator (`;`
/// or `\n`) at or after `off` (or end of input). A command line never
/// contains a separator, so this is the position of the byte that ends it.
/// Using the command line's own terminator (not the end of a multi-line
/// heredoc span) is what lets `unterminated` find a delimiter line that
/// follows the command line; a `;` terminator makes the run a hard parse
/// error, not a continuation.
fn cmd_line_end(line: &[u8], off: usize) -> usize {
    line.get(off..)
        .and_then(|s| s.iter().position(|&b| b == b'\n' || b == b';'))
        .map(|p| off + p)
        .unwrap_or(line.len())
}

#[cfg(test)]
mod tests;
