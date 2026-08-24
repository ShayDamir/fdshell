use crate::error::cmd::CmdError;
use crate::state::ShellState;
use error_stack::{Report, ResultExt};
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

const MAX_ALIAS_DEPTH: u32 = 16;
const RESERVED: &[&[u8]] = &[
    b"case", b"esac", b"if", b"fi", b"for", b"while", b"until", b"done",
];

/// Expand one command position, skipping fully quoted, double-quoted and
/// reserved words.
pub(super) fn expand_position(
    line: &[u8],
    position: (&ShortCStr, &usize, &usize, &bool),
    current: &mut ShortCStr,
    delta: &mut isize,
    cell: &ForkCell<ShellState>,
) -> Result<(), Report<CmdError>> {
    let (word0, s0, e0, fq) = position;
    let quoted = line.get(*s0..*e0).is_some_and(|r| r.contains(&b'"'));
    if *fq || quoted || RESERVED.iter().any(|r| word0.eq_bytes(r)) {
        return Ok(());
    }
    expand_at(current, delta, word0.clone(), *s0, *e0, cell)
}

/// Replace the word at `s0 + delta..e0 + delta` with its alias value,
/// chaining while the replacement is itself an alias. `delta` is the signed
/// length drift against the original token offsets; it goes negative when a
/// replacement is shorter than the word it replaces.
pub(super) fn expand_at(
    current: &mut ShortCStr,
    delta: &mut isize,
    mut word: ShortCStr,
    s0: usize,
    e0: usize,
    cell: &ForkCell<ShellState>,
) -> Result<(), Report<CmdError>> {
    let s = s0 as isize + *delta;
    let mut e = e0 as isize + *delta;
    for _ in 0..MAX_ALIAS_DEPTH {
        if RESERVED.iter().any(|r| word.eq_bytes(r)) {
            break;
        }
        let state = cell.borrow().change_context(CmdError::Never)?;
        let value = state.aliases.get(&word).cloned();
        drop(state);
        let Some(value) = value else {
            break;
        };
        let pre = current.get(..(s as usize)).ok_or(CmdError::Never)?;
        let post = current.get(e as usize..).ok_or(CmdError::Never)?;
        *delta += value.len() as isize - (e - s);
        *current = ShortCStr::concat(&[&pre, &value, &post]);
        e = s + value.len() as isize;
        word = value;
    }
    Ok(())
}
