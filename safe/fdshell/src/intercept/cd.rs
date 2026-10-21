//! `cd [dir]` — move the shell's base directory (the open `%CWD` fd).
//!
//! The operand is an ordinary **word**: it goes through the argument pipeline
//! (`substitute_args` — tilde, `$name`, `$( … )`, backticks, `$(( … ))`, `%fd`,
//! escape pairs, IFS splitting, pathname expansion), and the target is decided
//! on the **expanded fields** (each rule measured on bash 5.3.9):
//!
//! - 0 fields is the bare `cd`, i.e. `$HOME` (`cd "$@"` with no positionals,
//!   `cd $unset`, `shopt -s nullglob; cd zzz*`);
//! - 1 field is the directory, and the field `-` is the `OLDPWD` form — read on
//!   the expanded value, so `D=-; cd "$D"` goes back even where a directory
//!   named `-` exists (`cd ./-` is how that directory is reached);
//! - 2+ fields is `cd: too many arguments` and opens nothing (fdshell's rc is
//!   1 and the report aborts the script — the accepted task #179 family);
//! - `shopt -s strict` bans nothing here: the base follows the shell, and
//!   `cd ..` is the documented escape hatch (README "Strict mode").
//!
//! The one fdshell-only form, `%name`, is read on the **raw** word and never
//! expanded: the pipeline's `%` arm yields the fd *number*, so a `%dir` word
//! sent through it would become `cd 7`, and the stored fd's `Origin`/trace must
//! survive the move.

use alloc::vec::Vec;
use error_stack::{Report, ResultExt, bail};

use crate::cd::CdTarget;
use crate::error::cmd::CmdError;
use crate::state::ShellState;
use sys::ScriptText;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

pub(crate) fn run_cd(
    line: &[u8],
    cmdline: &crate::parse::CommandLine,
    text: &ScriptText,
    cell: &ForkCell<ShellState>,
) -> Result<bool, Report<CmdError>> {
    super::validation::validate_intercept(line, "cd", cmdline)?;
    // The expansion re-borrows the `ForkCell` (IFS, `dotglob`, `%fd`, and a
    // `$( … )` forks), so it runs **before** the state borrow, never across it
    // (LESSONS "A parent-side intercept's words are a substitution surface").
    let var = cmdline
        .args
        .first()
        .and_then(|word| word.strip_prefix(b"%"));
    let fields = match var {
        // The `%name` form is decided on the raw word, so no expansion runs at
        // all: `cd %NOPE` stays `fd variable not set`, and quoting is no gate
        // (`cd "%d"` is the fd form) — the gate is a raw-text test.
        Some(_) => Vec::new(),
        None => crate::substitute::substitute_args(
            &cmdline.args,
            &cmdline.args_mask,
            &cmdline.args_span,
            &cmdline.args_quoted,
            cell,
        )
        .change_context(CmdError::Resolve)?,
    };
    let target = target(var, &fields)?;
    let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
    crate::cd::cd(target, &mut state, text.start).change_context(CmdError::Cd)?;
    state.set_last_exit(0);
    Ok(true)
}

/// Decide which directory to open. `var` is the `%name` operand's name, read
/// off the raw word by the caller (`None` for every other word, whose fields
/// came out of the pipeline).
fn target(var: Option<ShortCStr>, fields: &[ShortCStr]) -> Result<CdTarget, Report<CmdError>> {
    if let Some(name) = var {
        return Ok(CdTarget::Var(name));
    }
    Ok(match fields {
        [] => CdTarget::Home,
        [field] if field.eq_bytes(b"-") => CdTarget::OldCwd,
        [field] => CdTarget::Path(field.clone()),
        _ => bail!(CmdError::CdTooManyArgs),
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests;
