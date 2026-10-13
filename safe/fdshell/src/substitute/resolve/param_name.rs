//! The parameter names that are not variables: an ASCII digit run is a
//! positional index (`0` is the program name), and the single bytes `#`, `?`,
//! `$`, `@`, `*`, `-` are the shell's special parameters. The bare `$` path and
//! the braced path share the index map and the nounset rule through this file
//! so the four expansion arms cannot drift (LESSONS: a positional parameter is
//! a parameter, not a variable name).

use core::fmt::Write;
use error_stack::{Report, ResultExt, bail};
use sys::ShortCStr;

use crate::error::resolve::ResolveError;
use crate::state::ShellState;
use crate::substitute::positional::positional_join;

/// The single-byte special parameters.
const SPECIALS: [u8; 6] = *b"#?$@*-";

impl ShellState {
    /// Value of a parameter name: a digit run is a positional, a special byte
    /// renders its own value, and `Ok(None)` means "an ordinary variable name,
    /// keep looking". `nounset` is passed in by the lookup: the pattern family
    /// consults `set -u`, the colon family is exempt.
    pub(crate) fn param_name_value(
        &self,
        name: &ShortCStr,
        nounset: bool,
    ) -> Result<Option<ShortCStr>, Report<ResolveError>> {
        if is_digit_run(name) {
            return self.positional_value(name, nounset);
        }
        match special_byte(name) {
            Some(byte) => self.special_value(byte).map(Some),
            None => Ok(None),
        }
    }

    /// `$0` is the program name and `$N` is `positional.get(N - 1)` (1-based
    /// parameters, 0-based storage). An out-of-range index is unset: empty, and
    /// under `set -u` the bail names the index verbatim as bash does.
    pub(crate) fn positional_value(
        &self,
        num: &ShortCStr,
        nounset: bool,
    ) -> Result<Option<ShortCStr>, Report<ResolveError>> {
        let idx: usize = num.parse().change_context(ResolveError::TooLarge)?;
        if idx == 0 {
            return Ok(Some(self.program_name.value.clone()));
        }
        match self.positional.get(idx - 1) {
            Some(pos) => Ok(Some(pos.value.clone())),
            None if nounset => bail!(ResolveError::UnboundVariable { var: num.clone() }),
            None => Ok(None),
        }
    }

    /// The value of a special parameter: a rendered number, the positional
    /// join, or the active option flags — never a borrow of the state.
    fn special_value(&self, byte: u8) -> Result<ShortCStr, Report<ResolveError>> {
        let mut out = ShortCStr::new();
        match byte {
            b'#' => core::write!(out, "{}", self.positional.len())
                .change_context(ResolveError::Never)?,
            b'?' => core::write!(out, "{}", self.last_status.exit_code())
                .change_context(ResolveError::Never)?,
            b'$' => core::write!(out, "{}", self.shell_pid).change_context(ResolveError::Never)?,
            b'-' => out
                .push_checked(&crate::options::flags(self.options))
                .change_context(ResolveError::NulByte)?,
            b'@' | b'*' => return positional_join(&self.positional, &self.ifs),
            _ => {}
        }
        Ok(out)
    }

    /// Does `name` name a parameter (a positional index, `0` included, or a
    /// special) rather than a variable? `:=` consults it before writing: a
    /// positional or special has no slot to write into.
    pub(crate) fn is_param_name(&self, name: &ShortCStr) -> bool {
        is_digit_run(name) || special_byte(name).is_some()
    }
}

/// A non-empty run of ASCII digits: every digit run is a positional index.
fn is_digit_run(name: &ShortCStr) -> bool {
    name.as_bytes()
        .is_ok_and(|b| !b.is_empty() && b.iter().all(u8::is_ascii_digit))
}

/// The special byte of `name`: only a single-byte name is a special parameter.
fn special_byte(name: &ShortCStr) -> Option<u8> {
    let bytes = name.as_bytes().ok()?;
    let byte = bytes.first().copied()?;
    (bytes.len() == 1 && SPECIALS.contains(&byte)).then_some(byte)
}
