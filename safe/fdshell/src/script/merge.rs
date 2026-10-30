//! The runner's join of a cond list that a here-doc region splits (task #492).
//!
//! POSIX #2.9 reads `&&`/`||` plus its newline as one token, so the list
//! continues onto the next physical line and the scanner joins the run there
//! (`segment.rs`). A physical line that claims a body region is the exception:
//! its body is a region on the lines below, and the region text cannot be both
//! inline in the run's bytes and attached in operator order (LESSONS: "A heredoc
//! body is a property of the logical line, not of a contiguous span"), so the
//! scanner leaves every physical line's segment as it stands, each keeping its
//! own (text, bodies) pair, and the runner concatenates them here — in operator
//! order — into the single statement the join calls for, spelled the way the whole
//! list on one physical line would be: [`chain`] writes the links on one line,
//! space separated, because the parse layer reads a newline as a `;` token (LESSONS:
//! "A `;` token is a newline in disguise") and an operator left at the end of a
//! synthesized line is no operator token for it — a chain of two or more operators
//! would split back into separate statements. A link whose own text spans physical
//! lines is written on the one line too: the scan left the offsets of its separator
//! newlines on the segment, and the chain writes each as a space (a newline that is
//! data — inside a quoted word, or the one ending a `#` comment — stays). The
//! scanner marks the segment it could not join
//! (`Segment::Statement::join_next`); nothing here re-reads the predicate, and a
//! `;`-separated pair never carries the mark.

use crate::error::cmd::CmdError;
use crate::segment::Segment;
use alloc::vec::Vec;
use error_stack::{Report, ResultExt};
use sys::{ScriptText, ShortCStr};

use self::chain::Part;

mod chain;

/// A segment as the runner sees it: the segment itself, and when it is a
/// statement that continues onto the segments below, the joined statement to
/// run in its place (`None` = run the segment as it stands).
pub(crate) struct Unit<'a> {
    pub(crate) segment: &'a Segment<'a>,
    pub(crate) joined: Option<Joined>,
}

/// A joined statement: its synthesized text (the physical lines' command texts,
/// in scan order — the region lines are not command bytes — on one line, space
/// separated, so the parse layer sees the operators as operators), its bodies in
/// operator order, and the plain-`&` flag of the list's **last** part (task
/// #326).
pub(crate) struct Joined {
    pub(crate) text: Vec<u8>,
    pub(crate) bodies: Vec<Vec<u8>>,
    pub(crate) background: bool,
}

/// The segments to execute, in order, with every chain of segments a
/// line-ending `&&`/`||` continues folded into its first member.
pub(crate) fn units<'a>(segments: &'a [Segment<'a>]) -> Vec<Unit<'a>> {
    let mut out: Vec<Unit<'_>> = Vec::new();
    let mut i = 0;
    while let Some(segment) = segments.get(i) {
        let Some(first) = chain::part(segment) else {
            out.push(Unit {
                segment,
                joined: None,
            });
            i += 1;
            continue;
        };
        let mut links: Vec<Part<'_>> = Vec::new();
        let mut link = Some(first);
        while let Some(current) = link {
            let continues = current.join_next;
            links.push(current);
            i += 1;
            link = continues
                .then(|| segments.get(i).and_then(chain::part))
                .flatten();
        }
        let joined = (links.len() > 1).then(|| chain::concatenate(&links));
        out.push(Unit { segment, joined });
    }
    out
}

/// The `ScriptText` of a joined statement, anchored at the first segment's
/// offset: it has no single span in the script, so its text is built here the
/// way `cond/part.rs::reconstruct` builds a part's.
pub(crate) fn text_at(
    bytes: &[u8],
    off: usize,
    text: &ScriptText,
) -> Result<ScriptText, Report<CmdError>> {
    let view = text.subslice(off, 1).ok_or(CmdError::Never)?;
    let data = ShortCStr::from_vec(bytes.to_vec()).change_context(CmdError::Never)?;
    Ok(ScriptText::new(data, view.start, view.origin.clone()))
}

#[cfg(test)]
mod tests;
