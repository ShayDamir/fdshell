#![forbid(unsafe_code)]

use sys::ShortCStr;
use sys::fork_cell::ForkCell;

use crate::state::ShellState;

pub(crate) fn expand_for_words(
    words: &[ShortCStr],
    cell: &ForkCell<ShellState>,
) -> Result<Vec<ShortCStr>, i32> {
    let mut out = Vec::new();
    for word in words {
        let bs = word.as_bytes()?;
        let split = if is_cmd_subst(bs) {
            let expanded = crate::cmd_subst::run_and_capture(strip_delims(bs), cell)?;
            split_whitespace(&expanded)?
        } else {
            vec![word.clone()]
        };
        out.extend(split);
    }
    Ok(out)
}

fn is_cmd_subst(bs: &[u8]) -> bool {
    (bs.first() == Some(&b'`') && bs.last() == Some(&b'`') && bs.len() >= 2)
        || (bs.len() >= 3 && bs.starts_with(b"$(") && bs.last() == Some(&b')'))
}

fn strip_delims(bs: &[u8]) -> &[u8] {
    if bs.starts_with(b"$(") {
        bs.get(2..bs.len() - 1).unwrap_or(b"")
    } else {
        bs.get(1..bs.len() - 1).unwrap_or(b"")
    }
}

fn split_whitespace(data: &[u8]) -> Result<Vec<ShortCStr>, i32> {
    let mut words = Vec::new();
    let mut cur = ShortCStr::new();
    for &b in data {
        if b == b' ' || b == b'\t' || b == b'\n' || b == b'\r' {
            if !cur.is_empty() {
                words.push(core::mem::take(&mut cur));
            }
        } else {
            cur.push(b)?;
        }
    }
    if !cur.is_empty() {
        words.push(cur);
    }
    Ok(words)
}
