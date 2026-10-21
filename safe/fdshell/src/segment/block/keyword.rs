//! The keyword block span (`if`/`for`/`while`/`until`/`case`/`wait`): the
//! closing-keyword scan and the heredoc-body region bookkeeping.

use super::Segment;
use super::length::opener_len;
use crate::comment::scan_block;
use crate::keywords::time_prefix_len;
use crate::scan::ScanState;
use crate::scan::heredoc;
use alloc::vec::Vec;

/// Build the `Segment::Block` for a block-opening keyword at `start`.
/// Returns the segment, the scan resume index, and the resume floor for the
/// rest of the physical line (past which no flush may resume).
/// `pre_resume` is the end of the body regions of a statement that precedes
/// the block on the same line (the line end when there is none); `line_start`
/// is the start of the physical line whose body stream the block reads.
///
/// The block claims **its own** regions of that line's body stream (see
/// [`heredoc::claimed_line_bodies`]), and the segment carries their bytes in
/// `bodies` when they sit entirely past the block's command text (**append
/// mode**): the span then stops at the closing keyword's separator, so a
/// trailing statement's text is outside the block, and `script.rs` hands the
/// body bytes to `run_cond_list` as the block's bodies — exactly like a
/// statement's (task #234), so the part that owns the `<<` operator is the one
/// that gets its body. The bodies are appended after the command line, in
/// operator order.
///
/// When the regions are interleaved with the block's own lines (a multi-line
/// block with a heredoc in its body), the segment carries no bodies and the
/// span's exclusive end (`end_pos`) is the closing keyword's separator
/// (`;`/`\n`) **extended forward** to the block's last claimed region: the span
/// must keep covering the whole block text. A trailing statement's bodies stay
/// outside the span, so a heredoc statement after the block on the same line is
/// not swallowed. The scan resumes just past the closing keyword (a trailing
/// statement on the same line is still scanned) and the floor keeps the line's
/// final flush from re-entering any body region; when the block ends the line
/// the resume is already past all of them.
pub(super) fn keyword_block<'a>(
    line: &'a [u8],
    raw: &[u8],
    part: &[u8],
    start: usize,
    line_start: usize,
    state: &ScanState,
    pre_resume: usize,
) -> (Segment<'a>, usize, usize) {
    let leading_ws = raw.iter().take_while(|&&b| b.is_ascii_whitespace()).count();
    // The keyword is scored on the word behind the `time [-p]` prefix run, so
    // `kw_len` measures the keyword itself (`time if …` must not measure `time`).
    let prefix = time_prefix_len(part).unwrap_or(0);
    let keyword = part.get(prefix..).unwrap_or(b"");
    let kw_len = opener_len(keyword).unwrap_or(0);
    let after_kw = start + leading_ws + prefix + kw_len;
    let mut quotes = state.quotes();
    let mut block_start_pos = after_kw;
    let (_end_pos, closed) = scan_block(line, after_kw, &mut quotes, &mut block_start_pos, 1);
    // The span's exclusive end: the closing keyword's separator (`;`/`\n`)
    // at `block_start_pos - 1` is excluded, or the line end when the closing
    // keyword is the last thing on the input (no separator to step past).
    let mut end = if block_start_pos >= line.len() {
        line.len()
    } else {
        block_start_pos - 1
    };
    let (resume, floor, bodies) = if closed {
        let line_end = heredoc::line_end_after(line, start);
        // `line_resume` is the resume floor's anchor: the end of the line's
        // last body region (the line end when the bodies are missing).
        let (_, line_resume) = heredoc::line_bodies_for_line(line, start, line_end);
        // The block's command text spans lines (a multi-line block's condition
        // body+terminator sit before its closer), so the count is the
        // window-blind one: those regions are heredoc structure, never the
        // block's operators. Regions claimed by an operator written **before**
        // this window are kept visible — a pre-block heredoc's region covers
        // the block's own line, and blinding it would hide the closer
        // (`tests::skip_is_bounded_to_the_blocks_physical_line`).
        let k = heredoc::operator_count_opaque(line, start, block_start_pos);
        // `k == 0` claims no region: skip the whole-line region scan for it.
        let bodies = if k == 0 {
            Vec::new()
        } else {
            let (claimed, contents) =
                heredoc::claimed_line_bodies(line, line_start, start, line_end, k);
            if claimed.first().is_some_and(|&(s, _)| s >= end) {
                contents
            } else {
                // Extension mode: forward-only — the span keeps the block text.
                end = match claimed.last() {
                    Some(&(_, e)) => end.max(e),
                    None => end.max(line_resume),
                };
                Vec::new()
            }
        };
        if block_start_pos < line_end {
            // Trailing content on the same line: scan it; the line's final
            // flush must not re-enter the body regions.
            (block_start_pos, line_resume.max(pre_resume), bodies)
        } else {
            // The block ends the line (or continues on later lines): resume
            // just past the closing keyword, never inside a body region.
            (block_start_pos.max(line_resume).max(pre_resume), 0, bodies)
        }
    } else {
        (end + 1, 0, Vec::new())
    };
    (
        Segment::Block {
            block_start: start,
            end_pos: end,
            closed,
            bodies,
        },
        resume,
        floor,
    )
}

#[cfg(test)]
mod tests;
