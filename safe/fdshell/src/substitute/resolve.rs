use error_stack::{Report, ResultExt};

use crate::error::resolve::ResolveError;
use crate::state::ShellState;
use sys::ShortCStr;

mod param_name;
mod param_value;

impl ShellState {
    /// Value of `name` in the shell's strings, then the inherited environment.
    /// `crate::arith` resolves arithmetic variables through the same lookup.
    pub(crate) fn var_value(&self, name: &ShortCStr) -> Option<&ShortCStr> {
        self.strings
            .get(name)
            .map(|v| &v.value)
            .or_else(|| self.environ.iter().find(|(k, _)| k == name).map(|(_, v)| v))
    }

    /// Indirect reference: expand `name`, then expand its value as a variable
    /// name. `content` is the whole `${…}` body, so the nounset message carries
    /// the `!` and matches bash's `!q: unbound variable`.
    pub(super) fn resolve_indirect(
        &self,
        content: &ShortCStr,
        out: &mut ShortCStr,
    ) -> Result<(), Report<ResolveError>> {
        if let Some(val) = self.param_value(content)? {
            out.push(&val);
        }
        Ok(())
    }

    pub(super) fn resolve_var_name(
        &self,
        name: &ShortCStr,
        out: &mut ShortCStr,
    ) -> Result<(), Report<ResolveError>> {
        if let Some(val) = self.param_value(name)? {
            out.push(&val);
        }
        Ok(())
    }
}

/// Expand `$N` from the digit run starting at `first_digit`: the index map,
/// `$0` and the nounset rule live in `positional_value`, shared with the braced
/// path so the two forms agree.
pub(super) fn resolve_positional_index(
    first_digit: u8,
    peek: &mut core::iter::Peekable<impl Iterator<Item = u8>>,
    state: &ShellState,
    out: &mut ShortCStr,
) -> Result<(), Report<ResolveError>> {
    let mut num = ShortCStr::new();
    num.push_byte(first_digit)
        .change_context(ResolveError::Never)?;
    while let Some(&nc) = peek.peek() {
        if nc.is_ascii_digit() {
            num.push_byte(nc).change_context(ResolveError::Never)?;
            peek.next();
        } else {
            break;
        }
    }
    let nounset = state.options & crate::options::NOUNSET != 0;
    if let Some(value) = state.positional_value(&num, nounset)? {
        out.push(&value);
    }
    Ok(())
}
