//! The shared braced-parameter lookup. The `!name` indirection lives in one
//! place so the operator families cannot drift; the `set -u` policy is a
//! parameter of it: the pattern family bails on an unset parameter, the colon
//! family is exempt because its own word supplies the value (bash
//! `set -u; ${undef:-x}` prints `x`).

use error_stack::{Report, bail};
use sys::ShortCStr;

use crate::error::resolve::ResolveError;
use crate::state::ShellState;

impl ShellState {
    /// The braced parameter's value, nounset-checked: `!name` is an indirect
    /// reference, an unbound name bails under `set -u`, and an unset parameter
    /// is `None` (POSIX 2.6.2 expands it to the empty string).
    pub(crate) fn param_value(
        &self,
        name: &ShortCStr,
    ) -> Result<Option<&ShortCStr>, Report<ResolveError>> {
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
    ) -> Result<Option<&ShortCStr>, Report<ResolveError>> {
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

    /// The `!` dispatch, `nounset_check` saying whether this family consults
    /// `set -u` at all: the pattern family does, the colon family does not.
    fn lookup(
        &self,
        name: &ShortCStr,
        nounset_check: bool,
    ) -> Result<Option<&ShortCStr>, Report<ResolveError>> {
        let nounset = nounset_check && self.options & crate::options::NOUNSET != 0;
        match name.strip_prefix(b"!") {
            Some(target_name) => self.indirect_value(name, &target_name, nounset),
            None => self.direct_value(name, nounset),
        }
    }

    /// A plain name: bound is the value, unbound is the nounset bail.
    fn direct_value(
        &self,
        name: &ShortCStr,
        nounset: bool,
    ) -> Result<Option<&ShortCStr>, Report<ResolveError>> {
        if let Some(val) = self.var_value(name) {
            return Ok(Some(val));
        }
        if nounset {
            bail!(ResolveError::UnboundVariable { var: name.clone() });
        }
        Ok(None)
    }

    /// `!name`: the value of `name` is the target parameter's name. `name` (the
    /// whole braced body, `!` included) is what nounset reports, as in bash's
    /// `!q: unbound variable`; an unset target is `None` for the colon family.
    fn indirect_value(
        &self,
        name: &ShortCStr,
        target_name: &ShortCStr,
        nounset: bool,
    ) -> Result<Option<&ShortCStr>, Report<ResolveError>> {
        match self.var_value(self.indirect_target(target_name)?) {
            Some(val) => Ok(Some(val)),
            None if nounset => bail!(ResolveError::UnboundVariable { var: name.clone() }),
            None => Ok(None),
        }
    }

    /// The name of the parameter a `!name` reference points at: bash's
    /// `undefined: invalid indirect expansion` when `name` is unbound, and its
    /// `: invalid variable name` when `name` is bound to the empty string —
    /// fdshell keeps both rcs and names its own target (`: invalid indirect
    /// expansion`, task #172).
    fn indirect_target(&self, target_name: &ShortCStr) -> Result<&ShortCStr, Report<ResolveError>> {
        match self.var_value(target_name) {
            None => bail!(ResolveError::InvalidIndirect {
                var: target_name.clone()
            }),
            Some(target) if target.is_empty() => {
                bail!(ResolveError::InvalidIndirect {
                    var: target.clone()
                })
            }
            Some(target) => Ok(target),
        }
    }
}
