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
//!   named `-` exists (`cd ./-` is how that directory is reached), and after a
//!   **successful** move that form writes the new directory to stdout (POSIX XCU
//!   4.57) — the value its own `pwd` would print there;
//! - 2+ fields is `cd: too many arguments` and opens nothing (fdshell's rc is
//!   1 and the report aborts the script — the accepted task #179 family);
//! - `shopt -s strict` bans nothing here: the base follows the shell, and
//!   `cd ..` is the documented escape hatch (README "Strict mode").
//!
//! Option words are parsed on the **expanded fields** (LESSONS #279 item 3),
//! from the head of the list: `--`, `-L`, `-P` and `-e` are accepted, the last
//! three as no-ops, `--` is consumed and **ends** the parsing, a lone `-` is not
//! an option but the `OLDPWD` operand, and the first other word starts the
//! operand list. bash accepts clustered shorts (`-LP sub`) and names the first
//! bad character (`---` is reported as `--`); fdshell matches the four words
//! exactly and names the whole word — pinned divergences (README).
//!
//! The one fdshell-only form, `%name`, is read on the **raw** word and never
//! expanded: the pipeline's `%` arm yields the fd *number*, so a `%dir` word
//! sent through it would become `cd 7`, and the stored fd's `Origin`/trace must
//! survive the move. The gate is a **word 0** rule, so in `cd -L %dir` the `%`
//! word goes through the pipeline like any other argument.

use alloc::vec::Vec;
use error_stack::{Report, ResultExt, bail};

use crate::cd::CdTarget;
use crate::error::cmd::CmdError;
use crate::state::ShellState;
use sys::OUT;
use sys::ScriptText;
use sys::ShortCStr;
use sys::env::getcwd;
use sys::fork_cell::ForkCell;

/// The operand's field list travels back to the frame, so `$_` is the last
/// **expanded** field; the `%name` path reports none and keeps the raw rule.
/// The **whole** list travels, option words included, because bash sets `$_`
/// from the last word it read (`cd --` leaves `--`, `cd -L sub` leaves `sub`).
pub(crate) fn run_cd(
    line: &[u8],
    cmdline: &crate::parse::CommandLine,
    text: &ScriptText,
    cell: &ForkCell<ShellState>,
) -> Result<Option<Vec<ShortCStr>>, Report<CmdError>> {
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
    let expanded = var.is_none();
    let target = target(var, &fields)?;
    // Bound before `target` moves into `cd` (the enum is not `Copy`): the print is
    // a per-form behavior, so the gate lives here, not in `cd.rs`.
    let dashed = matches!(target, CdTarget::OldCwd);
    let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
    crate::cd::cd(target, &mut state, text.start).change_context(CmdError::Cd)?;

    // Only the `-` form writes, and only a move that **succeeded** (a failed one
    // prints nothing, as bash). The print runs before `set_last_exit(0)` so a
    // write failure is never reported as success.
    if dashed {
        print_new_directory()?;
    }

    state.set_last_exit(0);
    Ok(expanded.then_some(fields))
}

/// The `-` form's line, in the exact shape of `child/simple.rs::handle_pwd`:
/// `getcwd` is the shell's single source of the current directory's path, since
/// a stored `Origin` is provenance — `Shell`, `EnvVar(HOME)`, the relative name a
/// hop was typed with — not a printable path
/// (LESSONS: "The cwd's printable path is `getcwd`"). bash prints its own `PWD`
/// value here; the two are the same bytes unless a hop went through a symlink,
/// and `cd`'s `O_NOFOLLOW` open means fdshell never takes one.
fn print_new_directory() -> Result<(), Report<CmdError>> {
    let cwd = getcwd().change_context(CmdError::CdDashWrite)?;
    OUT.write_all(&cwd).change_context(CmdError::CdDashWrite)?;
    OUT.write_all(b"\n").change_context(CmdError::CdDashWrite)?;
    Ok(())
}

/// Decide which directory to open. `var` is the `%name` operand's name, read
/// off the raw word by the caller (`None` for every other word, whose fields
/// came out of the pipeline). The match runs on the **operands**: the field list
/// with its leading option words consumed by [`operands`].
fn target(var: Option<ShortCStr>, fields: &[ShortCStr]) -> Result<CdTarget, Report<CmdError>> {
    if let Some(name) = var {
        return Ok(CdTarget::Var(name));
    }
    let fields = operands(fields)?;
    Ok(match fields {
        [] => CdTarget::Home,
        [field] if field.eq_bytes(b"-") => CdTarget::OldCwd,
        [field] => CdTarget::Path(field.clone()),
        _ => bail!(CmdError::CdTooManyArgs),
    })
}

/// Walk the leading option words of the **expanded** field list and return the
/// operand list. `-L`, `-P` and `-e` are accepted as no-ops: the move is a pinned
/// `O_DIRECTORY + O_NOFOLLOW` fd (README "The accepted divergences"), so the
/// logical/physical distinction bash measures on a symlinked directory has
/// nothing to decide here, and the conflict bash accepts is accepted too.
fn operands(fields: &[ShortCStr]) -> Result<&[ShortCStr], Report<CmdError>> {
    let mut rest = fields;
    while let Some(word) = rest.first() {
        if is_noop_option(word) {
            rest = rest.get(1..).unwrap_or(&[]);
            continue;
        }
        if word.eq_bytes(b"--") {
            return Ok(rest.get(1..).unwrap_or(&[]));
        }
        if word.eq_bytes(b"-") || !word.starts_with(b"-") {
            return Ok(rest);
        }
        bail!(CmdError::CdInvalidOption {
            option: word.clone()
        });
    }
    Ok(rest)
}

/// The accepted words, matched whole — no getopt cluster walker (decision pinned
/// in README: bash's help line `[-@]` is the getopt `@` marker, not an option).
fn is_noop_option(word: &ShortCStr) -> bool {
    word.eq_bytes(b"-L") || word.eq_bytes(b"-P") || word.eq_bytes(b"-e")
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests;
