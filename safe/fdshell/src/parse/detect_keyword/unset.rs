//! `unset`'s operand list (task #290): every word after the keyword is a name.
//!
//! A leading `%` selects the fd namespace (`%name`, and the array-entry form
//! `%arr[%src]`); any other word is a **shell** variable name, the spelling
//! POSIX `unset name…` takes. The detector is infallible: bash 5.3.9 validates
//! nothing here (`unset 1abc`, `unset a-b`, `unset ""`, `unset 1`, `unset -x`,
//! `unset -v v` are all rc 0, the ones naming no variable silently), so every
//! word is a target and an unbound name is a run-time no-op. A rejection at
//! this layer would be a statement-level loss (LESSONS, task #290).

use crate::parse::Token;
use crate::parse::array_ref::split_element_ref;
use crate::parse::line::{ParsedLine, UnsetTarget};
use sys::ShortCStr;

/// Parse `unset`'s operands, token 0 being the keyword itself.
pub(crate) fn detect(tokens: &[Token]) -> ParsedLine {
    let targets = tokens
        .iter()
        .skip(1)
        .map(|token| target(&token.0))
        .collect();
    ParsedLine::Unset(targets)
}

/// One operand word: `%`-prefixed words name the fd namespace, everything else
/// is a shell variable. A malformed array tail stays a plain fd name
/// (`%arr[conn]`), the rule this keyword has always had.
fn target(word: &ShortCStr) -> UnsetTarget {
    let Some(name) = word.strip_prefix(b"%") else {
        return UnsetTarget::Var(word.clone());
    };
    match split_element_ref(&name) {
        Some((var, source)) => UnsetTarget::FdArrayEntry { var, source },
        None => UnsetTarget::Fd(name),
    }
}

#[cfg(test)]
mod tests;
