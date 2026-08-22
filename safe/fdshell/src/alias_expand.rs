use crate::error::cmd::CmdError;
use crate::state::ShellState;
use error_stack::{Report, ResultExt};
use sys::ScriptText;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

const MAX_ALIAS_DEPTH: u32 = 16;
const RESERVED: &[&[u8]] = &[
    b"case", b"esac", b"if", b"fi", b"for", b"while", b"until", b"done",
];

/// Replace the leading word of the line with its alias value (chained up to
/// `MAX_ALIAS_DEPTH`), when `expand_aliases` is on.
pub(crate) fn expand_alias(
    text: &ScriptText,
    cell: &ForkCell<ShellState>,
) -> Result<ScriptText, Report<CmdError>> {
    let state = cell.borrow().change_context(CmdError::Never)?;
    if state.options & crate::options::EXPAND_ALIASES == 0 {
        return Ok(text.clone());
    }
    let mut current =
        ShortCStr::from_vec(text.as_bytes().change_context(CmdError::Never)?.to_vec())
            .change_context(CmdError::Never)?;
    for _ in 0..MAX_ALIAS_DEPTH {
        let Some(word) = first_word(&current) else {
            break;
        };
        if RESERVED.iter().any(|r| word.eq_bytes(r)) {
            break;
        }
        let state = cell.borrow().change_context(CmdError::Never)?;
        let Some(value) = state.aliases.get(&word) else {
            break;
        };
        let rest = current
            .get(word.len()..)
            .ok_or(CmdError::Never)
            .change_context(CmdError::Never)?;
        current = join(value, &rest)?;
    }
    Ok(ScriptText::new(current, text.start, text.origin.clone()))
}

fn first_word(line: &ShortCStr) -> Option<ShortCStr> {
    let bytes = line.as_bytes().ok()?;
    let start = bytes.iter().position(|&b| !b.is_ascii_whitespace())?;
    let rest = bytes.get(start..)?;
    let len = rest
        .iter()
        .take_while(|&&b| !b.is_ascii_whitespace())
        .count();
    line.get(start..start + len)
}

fn join(value: &ShortCStr, rest: &ShortCStr) -> Result<ShortCStr, Report<CmdError>> {
    let mut out = ShortCStr::new();
    out.push_checked(value.as_bytes().change_context(CmdError::Never)?)
        .change_context(CmdError::Never)?;
    out.push_checked(rest.as_bytes().change_context(CmdError::Never)?)
        .change_context(CmdError::Never)?;
    Ok(out)
}
