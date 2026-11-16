// mutants: -C --lib -- -E 'test(/^intercept::getopts::/)'

//! The pure per-word scan (task #590): one `getopts` call looks at the word the
//! counter names, reports one option character, and hands back the position the
//! next call starts from. The scan is a pure function so every measured shape of
//! the plan §2 is pinned in-process; the driver in [`super`] only resolves the
//! word list and commits the counters.

use sys::ShortCStr;

use super::byte_at;
use super::optstring::Optstring;

/// What one scan step found. `Presentation` carries everything task #591 needs
/// (it owns naming, diagnostics and statuses) and nothing that only this task
/// needs; the end-of-options status is the driver's (bash: 1).
#[derive(Debug, PartialEq)]
pub(crate) enum Scan {
    Found {
        ch: u8,
        arg: Option<ShortCStr>,
        next_index: u64,
        next_char: usize,
    },
    Presentation {
        ch: u8,
        problem: Problem,
        next_index: u64,
        next_char: usize,
    },
    End {
        index: u64,
    },
}

/// A presentation problem of the scan result (unknown option / missing
/// option-argument); its variable semantics are task #591's.
#[derive(Debug, PartialEq)]
pub(crate) enum Problem {
    Unknown,
    MissingArgument,
}

/// Scan one word from `cursor` (0 = word start, else the byte offset of the next
/// option character within it) at the 1-based `index`, given the parsed
/// `optstring` and the word that follows (`None` = past the end).
pub(crate) fn scan(
    index: u64,
    word: &ShortCStr,
    cursor: usize,
    opts: &Optstring,
    next_word: Option<&ShortCStr>,
) -> Scan {
    let offset = if cursor == 0 { 1 } else { cursor };
    if cursor == 0 {
        // End-of-options decisions: a word that is not an option word, the lone
        // `-` (stays on its own index, bash measures), and the `--` terminator
        // (consumed: the answer names the word after it).
        if !word.starts_with(b"-") || word.eq_bytes(b"-") {
            return Scan::End { index };
        }
        if word.eq_bytes(b"--") {
            return Scan::End { index: index + 1 };
        }
    }
    let Some(ch) = byte_at(word, offset) else {
        // The word at this index changed under a mid-cluster cursor; the scan
        // has no character here to report, so the list ends on it.
        return Scan::End { index };
    };
    let rest = word.get(offset + 1..);
    let word_done = rest.as_ref().is_some_and(ShortCStr::is_empty);
    match opts.lookup(ch) {
        None => presentation(ch, Problem::Unknown, index, offset, word_done),
        Some(true) => match (rest.filter(|rest| !rest.is_empty()), next_word) {
            // The option-argument is the attached rest of the word first, the
            // whole next word otherwise — and a word consumed as an
            // option-argument moves the counter too (`set -- -ab -b val`).
            (Some(arg), _) => found(ch, Some(arg), index + 1, 0),
            (None, Some(next)) => found(ch, Some(next.clone()), index + 2, 0),
            (None, None) => presentation(ch, Problem::MissingArgument, index, offset, word_done),
        },
        // A flag never takes the next word; it reports and the word keeps its
        // remaining cluster characters (measured `set -- -c val`).
        Some(false) if word_done => found(ch, None, index + 1, 0),
        Some(false) => found(ch, None, index, offset + 1),
    }
}

/// The one-line decision of the scan (task #590, for task #591 and the docs):
/// after a presentation error the scan reports the problem and **continues** —
/// bash 5.3.9 measures `set -- -a -x -b val` as `a`, the `-x` error, `b`=`val`
/// (and a word the error ends moves the counter to the next word); POSIX would
/// end option processing at the error, and flipping this one arm is that rule.
fn presentation(ch: u8, problem: Problem, index: u64, offset: usize, word_done: bool) -> Scan {
    if word_done {
        presentation_at(ch, problem, index + 1, 0)
    } else {
        presentation_at(ch, problem, index, offset + 1)
    }
}

/// The scan's two constructors: same fields, so the match arms stay one line.
fn found(ch: u8, arg: Option<ShortCStr>, next_index: u64, next_char: usize) -> Scan {
    Scan::Found {
        ch,
        arg,
        next_index,
        next_char,
    }
}

fn presentation_at(ch: u8, problem: Problem, next_index: u64, next_char: usize) -> Scan {
    Scan::Presentation {
        ch,
        problem,
        next_index,
        next_char,
    }
}

#[cfg(test)]
mod tests;
