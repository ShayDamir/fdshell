//! The runner's join of a cond list that a here-doc region splits (task #492).
//!
//! POSIX #2.9 reads `&&`/`||` plus its newline as one token, so the list
//! continues onto the next physical line and the scanner joins the run there
//! (`segment.rs`). A run whose own command line wrote a `<<` is the exception:
//! its body is a region on the lines below, and the region text cannot be both
//! inline in the run's bytes and attached in operator order (LESSONS: "A heredoc
//! body is a property of the logical line, not of a contiguous span"), so the
//! scanner leaves every physical line's segment as it stands, each keeping its
//! own (text, bodies) pair, and the runner concatenates them here — in operator
//! order — into the single statement the join calls for. The scanner marks the
//! segment it could not join (`Segment::Statement::join_next`); nothing here
//! re-reads the predicate, and a `;`-separated pair never carries the mark.

use crate::error::cmd::CmdError;
use crate::segment::Segment;
use alloc::vec::Vec;
use error_stack::{Report, ResultExt};
use sys::{ScriptText, ShortCStr};

/// A segment as the runner sees it: the segment itself, and when it is a
/// statement that continues onto the segments below, the joined statement to
/// run in its place (`None` = run the segment as it stands).
pub(crate) struct Unit<'a> {
    pub(crate) segment: &'a Segment<'a>,
    pub(crate) joined: Option<Joined>,
}

/// A joined statement: its synthesized text (the physical lines' command texts,
/// in scan order — the region lines are not command bytes), its bodies in
/// operator order, and the plain-`&` flag of the list's **last** part (task
/// #326).
pub(crate) struct Joined {
    pub(crate) text: Vec<u8>,
    pub(crate) bodies: Vec<Vec<u8>>,
    pub(crate) background: bool,
}

/// One statement's parts; a non-statement is no link of a chain.
struct Part<'a> {
    text: &'a [u8],
    bodies: &'a Vec<Vec<u8>>,
    background: bool,
    join_next: bool,
}

/// The segments to execute, in order, with every chain of segments a
/// line-ending `&&`/`||` continues folded into its first member.
pub(crate) fn units<'a>(segments: &'a [Segment<'a>]) -> Vec<Unit<'a>> {
    let mut out: Vec<Unit<'_>> = Vec::new();
    let mut i = 0;
    while let Some(segment) = segments.get(i) {
        let Some(first) = part(segment) else {
            out.push(Unit {
                segment,
                joined: None,
            });
            i += 1;
            continue;
        };
        let mut chain: Vec<Part<'_>> = Vec::new();
        let mut link = Some(first);
        while let Some(current) = link {
            let continues = current.join_next;
            chain.push(current);
            i += 1;
            link = continues.then(|| segments.get(i).and_then(part)).flatten();
        }
        let joined = (chain.len() > 1).then(|| concatenate(&chain));
        out.push(Unit { segment, joined });
    }
    out
}

/// The chain as one statement: the texts in scan order, the bodies in operator
/// order, and the last part's flag.
fn concatenate(chain: &[Part<'_>]) -> Joined {
    let mut text = Vec::new();
    let mut bodies = Vec::new();
    for (i, link) in chain.iter().enumerate() {
        if i > 0 {
            text.push(b'\n');
        }
        text.extend_from_slice(link.text);
        bodies.extend(link.bodies.iter().cloned());
    }
    let background = chain.last().map(|p| p.background).unwrap_or(false);
    Joined {
        text,
        bodies,
        background,
    }
}

/// A segment's text, bodies and flag, or `None` for anything but a statement.
fn part<'a>(segment: &'a Segment<'a>) -> Option<Part<'a>> {
    match segment {
        Segment::Statement {
            cmd,
            bodies,
            background,
            join_next,
            ..
        } => Some(Part {
            text: cmd,
            bodies,
            background: *background,
            join_next: *join_next,
        }),
        _ => None,
    }
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
