//! The plain `&` background boundary (task #326): the predicate the boundary
//! scan classifies at a byte. "bg" names the plain `&` async operator —
//! `async` is a Rust reserved word, so the module takes the shell name
//! (LESSONS: "`become` is a reserved keyword — rename modules").
//! Statement *validation* lives in `segment/bg.rs`.

use super::ScanState;

/// `true` when the byte `&` at `i` (bare, paren depth 0, outside quotes and
/// backticks — checked by the caller) is the plain async operator. Glue
/// rules first: not glued to a following word character (a task-name word
/// `&x`: `waitpid &x` — fdshell's name convention), not starting `&&`/`&>`
/// (the `&>` word unit: `&>`/`&>>`/`&>&n`), not glued to a preceding
/// operator byte (`&&`'s second byte, the `N>&file` / `N>&-` / `|&` / `<&` /
/// `(&` / `)&` family), and not owned by a `>`/`<` written before it across
/// spaces (bash's `>&` operator token: `cat > &f` is a redirect, never a
/// split).
pub(super) fn is_bg(line: &[u8], i: usize, _state: &ScanState) -> bool {
    line.get(i) == Some(&b'&')
        && !glued_before(i.checked_sub(1).and_then(|j| line.get(j)))
        && !glued_after(line, i)
        && token_end(line, i)
        && !redirect_ahead(line, i)
}

/// The `&` is glued to the operator family before it: `&&`'s second byte,
/// the `N>&file` / `N>&-` / `|&` / `<&` redirect operators (they continue
/// through the `&`), and `(&` / `)&` inside `( … )`.
fn glued_before(prev: Option<&u8>) -> bool {
    matches!(
        prev,
        Some(&b'&') | Some(&b'>') | Some(&b'<') | Some(&b'|') | Some(&b'(') | Some(&b')')
    )
}

fn glued_after(line: &[u8], i: usize) -> bool {
    matches!(line.get(i + 1), Some(&b'&') | Some(&b'>'))
}

/// `true` when the byte after the `&` is **not** a word character: the `&`
/// stands alone (whitespace, `;`, `|`, end of input) and is the async
/// operator. A glued following word character makes it a task-name word
/// (`&x`: `waitpid &x`, `unset &x` — fdshell's name convention, LESSONS);
/// `&&`/`&>` are the operator family (owned by `glued_after`).
fn token_end(line: &[u8], i: usize) -> bool {
    matches!(
        line.get(i + 1),
        None | Some(b' ' | b'\t' | b'\n' | b';' | b'|' | b'&' | b'>' | b'<' | b'(' | b')' | b'\\')
    )
}

/// Bash tokenizes `>&` as one operator token across spaces (`cat > &f` is a
/// `>` redirect, never an async split; fdshell's `>` operand rule rejects
/// the `&` word with "invalid redirect"). Scan back over spaces/tabs: a
/// redirect operator there owns the `&`.
fn redirect_ahead(line: &[u8], i: usize) -> bool {
    let mut j = i.saturating_sub(1);
    while matches!(line.get(j), Some(&b' ') | Some(&b'\t')) {
        j = j.saturating_sub(1);
    }
    matches!(line.get(j), Some(&b'>') | Some(&b'<'))
}

#[cfg(test)]
mod tests;
