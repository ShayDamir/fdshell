//! The shared braced-parameter lookup. The unset policy (`set -u` bail / POSIX
//! empty) and the `!name` indirection live in one place so the arms cannot
//! drift (LESSONS: the unset policy lives in the shared lookup).

use error_stack::{Report, bail};
use sys::ShortCStr;

use crate::error::resolve::ResolveError;
use crate::state::ShellState;

impl ShellState {
    /// The braced parameter's value: `!name` is an indirect reference, an
    /// unbound name bails under `set -u`, and an unset parameter is `None`
    /// (POSIX 2.6.2 expands it to the empty string). The colon operators keep
    /// `var_value`, because their word supplies the value: nounset-exempt.
    pub(crate) fn param_value(
        &self,
        name: &ShortCStr,
    ) -> Result<Option<&ShortCStr>, Report<ResolveError>> {
        match name.strip_prefix(b"!") {
            Some(target_name) => self.indirect_value(name, &target_name),
            None => self.direct_value(name),
        }
    }

    /// A plain name: bound is the value, unbound is the nounset bail.
    fn direct_value(&self, name: &ShortCStr) -> Result<Option<&ShortCStr>, Report<ResolveError>> {
        if let Some(val) = self.var_value(name) {
            return Ok(Some(val));
        }
        if self.options & crate::options::NOUNSET != 0 {
            bail!(ResolveError::UnboundVariable { var: name.clone() });
        }
        Ok(None)
    }

    /// `!name`: the value of `name` is the target parameter's name. `name`
    /// (the whole braced body, `!` included) is what nounset reports, as in
    /// bash's `!q: unbound variable`; an unbound `name` is bash's `undefined:
    /// invalid indirect expansion`, and a `name` bound to the empty string
    /// names no target.
    fn indirect_value(
        &self,
        name: &ShortCStr,
        target_name: &ShortCStr,
    ) -> Result<Option<&ShortCStr>, Report<ResolveError>> {
        match self.var_value(target_name) {
            None => bail!(ResolveError::InvalidIndirect {
                var: target_name.clone()
            }),
            Some(target) if target.is_empty() => {
                bail!(ResolveError::InvalidIndirect {
                    var: target.clone()
                })
            }
            Some(target) => match self.var_value(target) {
                Some(val) => Ok(Some(val)),
                None if self.options & crate::options::NOUNSET != 0 => {
                    bail!(ResolveError::UnboundVariable { var: name.clone() });
                }
                None => Ok(None),
            },
        }
    }
}
