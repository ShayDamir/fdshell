use crate::error::cmd::CmdError;
use crate::parse::{Token, token::tokenize};
use crate::state::ShellState;
use error_stack::{Report, ResultExt};
use sys::ScriptText;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

const MAX_ALIAS_DEPTH: u32 = 16;
const RESERVED: &[&[u8]] = &[
    b"case", b"esac", b"if", b"fi", b"for", b"while", b"until", b"done",
];

/// Expand aliases at every command position of the line — the first word and
/// the word after each `|` — chaining up to `MAX_ALIAS_DEPTH` per position,
/// when `expand_aliases` is on.
pub(crate) fn expand_alias(
    text: &ScriptText,
    cell: &ForkCell<ShellState>,
) -> Result<ScriptText, Report<CmdError>> {
    let state = cell.borrow().change_context(CmdError::Never)?;
    if state.options & crate::options::EXPAND_ALIASES == 0 {
        return Ok(text.clone());
    }
    drop(state);
    let line = text.as_bytes().change_context(CmdError::Never)?;
    let tokens = tokenize(line).change_context(CmdError::Parse)?;
    let mut current = ShortCStr::from_vec(line.to_vec()).change_context(CmdError::Never)?;
    let mut delta = 0usize;
    for (word0, s0, e0, fq) in command_positions(&tokens) {
        let quoted = line.get(*s0..*e0).is_some_and(|r| r.contains(&b'"'));
        if *fq || quoted || RESERVED.iter().any(|r| word0.eq_bytes(r)) {
            continue;
        }
        expand_at(&mut current, &mut delta, word0.clone(), *s0, *e0, cell)?;
    }
    Ok(ScriptText::new(current, text.start, text.origin.clone()))
}

/// The command words of the line: the first token and the token following
/// each `|` token, as `(word, start, end, fully_quoted)`.
fn command_positions(tokens: &[Token]) -> alloc::vec::Vec<(&ShortCStr, &usize, &usize, &bool)> {
    let mut out = alloc::vec::Vec::new();
    if let Some(first) = tokens.first() {
        out.push((&first.0, &first.1, &first.2, &first.3));
    }
    for (i, (t, _, _, _)) in tokens.iter().enumerate() {
        if t.eq_bytes(b"|")
            && let Some(n) = tokens.get(i + 1)
        {
            out.push((&n.0, &n.1, &n.2, &n.3));
        }
    }
    out
}

/// Replace the word at `s0 + delta..e0 + delta` with its alias value,
/// chaining while the replacement is itself an alias. `delta` tracks the
/// length drift against the original token offsets.
fn expand_at(
    current: &mut ShortCStr,
    delta: &mut usize,
    mut word: ShortCStr,
    s0: usize,
    e0: usize,
    cell: &ForkCell<ShellState>,
) -> Result<(), Report<CmdError>> {
    let (s, mut e) = (s0 + *delta, e0 + *delta);
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
        let pre = current
            .get(..s)
            .ok_or(CmdError::Never)
            .change_context(CmdError::Never)?;
        let post = current
            .get(e..)
            .ok_or(CmdError::Never)
            .change_context(CmdError::Never)?;
        let replaced = ShortCStr::concat(&[&pre, &value, &post]);
        *delta += value.len() - (e - s);
        *current = replaced;
        e = s + value.len();
        word = value;
    }
    Ok(())
}
