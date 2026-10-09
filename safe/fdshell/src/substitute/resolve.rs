use error_stack::{Report, ResultExt, bail};

use crate::error::resolve::ResolveError;
use crate::state::ShellState;
use sys::ShortCStr;

impl ShellState {
    /// Value of `name` in the shell's strings, then the inherited environment.
    /// `crate::arith` resolves arithmetic variables through the same lookup.
    pub(crate) fn var_value(&self, name: &ShortCStr) -> Option<&ShortCStr> {
        self.strings
            .get(name)
            .map(|v| &v.value)
            .or_else(|| self.environ.iter().find(|(k, _)| k == name).map(|(_, v)| v))
    }

    /// Indirect reference: expand `name`, then expand its value as a variable name.
    /// `content` is the whole `${…}` body, so the nounset message carries the
    /// `!` and matches bash's `!q: unbound variable`.
    pub(super) fn resolve_indirect(
        &self,
        content: &ShortCStr,
        out: &mut ShortCStr,
    ) -> Result<(), Report<ResolveError>> {
        // The caller reached this arm by stripping the `!`, so it is present.
        let name = content.strip_prefix(b"!").ok_or(ResolveError::Never)?;
        match self.var_value(&name) {
            // bash: `undefined: invalid indirect expansion` (rc 1).
            None => bail!(ResolveError::InvalidIndirect { var: name.clone() }),
            // A name bound to the empty string names no target: bash says
            // `: invalid variable name` (same rc, its own wording).
            Some(target) if target.is_empty() => {
                bail!(ResolveError::InvalidIndirect {
                    var: target.clone()
                })
            }
            Some(target) => match self.var_value(target) {
                Some(val) => out.push(val),
                // nounset uses the full content, so the message is bash's
                // `!q: unbound variable`.
                None if self.options & crate::options::NOUNSET != 0 => {
                    bail!(ResolveError::UnboundVariable {
                        var: content.clone()
                    });
                }
                None => {}
            },
        }
        Ok(())
    }

    pub(super) fn resolve_var_name(
        &self,
        name: &ShortCStr,
        out: &mut ShortCStr,
    ) -> Result<(), Report<ResolveError>> {
        match self.var_value(name) {
            Some(val) => out.push(val),
            None => {
                // nounset: an unbound variable is an error, not an empty value.
                if self.options & crate::options::NOUNSET != 0 {
                    bail!(ResolveError::UnboundVariable { var: name.clone() });
                }
                // POSIX 2.6.2: an unset parameter expands to the empty string.
            }
        }
        Ok(())
    }
}

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
    let idx: usize = num.parse().change_context(ResolveError::TooLarge)?;
    match state.positional.get(idx) {
        Some(pos) => out.push(pos),
        // nounset: an out-of-range `$N` is an unbound variable (bash: `N: unbound variable`).
        None if state.options & crate::options::NOUNSET != 0 => {
            bail!(ResolveError::UnboundVariable { var: num });
        }
        None => {}
    }
    Ok(())
}
