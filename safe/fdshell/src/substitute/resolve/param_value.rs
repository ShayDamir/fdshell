//! The braced-parameter families' entry points, one per family: each keeps its
//! own `set -u` policy and hands every name it is given to the shared lookup in
//! [`lookup`], so no family re-implements the `!name` rule and none can drift.

// mutants: -C --lib -C --test -C colon -C --test -C positional -C --test -C pattern -- -E 'test(/^substitute::/) | binary(colon) | binary(positional) | binary(pattern)'

use error_stack::Report;
use sys::ShortCStr;

use crate::error::resolve::ResolveError;
use crate::state::ShellState;

mod lookup;

impl ShellState {
    /// The braced parameter's value, nounset-checked: positional and special
    /// parameters resolve first, `!name` is an indirect reference, an unbound
    /// name bails under `set -u`, and an unset parameter is `None` (POSIX 2.6.2
    /// expands it to the empty string).
    pub(crate) fn param_value(
        &self,
        name: &ShortCStr,
    ) -> Result<Option<ShortCStr>, Report<ResolveError>> {
        self.lookup(name, true)
    }

    /// The colon operators' value (`:-` `:=` `:+` `:?`): the same `!name` rule,
    /// nounset-exempt — their word supplies the value, so an unbound parameter
    /// is `None` and never a bail. An unresolvable indirect reference still
    /// bails: no word rescues a `!name` whose name is unbound or empty. The
    /// empty indirect name (`${!:-z}`) is unset, as in bash.
    pub(crate) fn colon_value(
        &self,
        name: &ShortCStr,
    ) -> Result<Option<ShortCStr>, Report<ResolveError>> {
        if name.strip_prefix(b"!").is_some_and(|t| t.is_empty()) {
            return Ok(None);
        }
        self.lookup(name, false)
    }

    /// The parameter `:=` writes: a plain name writes itself, `!name` writes the
    /// target its value names (bash assigns `${!v:=q}` to `x` when `v=x`), and
    /// an unresolvable reference bails like the read path, leaving no write.
    pub(crate) fn colon_target(&self, name: &ShortCStr) -> Result<ShortCStr, Report<ResolveError>> {
        Ok(match name.strip_prefix(b"!") {
            Some(target_name) if !target_name.is_empty() => {
                self.indirect_target(&target_name)?.clone()
            }
            _ => name.clone(),
        })
    }
}
