// mutants: -C --lib -C --test -C repl -- -E 'test(/^repl::/) | binary(repl)'

//! The display rules behind `cwd.rs` (task #619): origin → path (D-619-1/2),
//! the `~` branch off the shell's own `HOME` lookup — `var_value`, the same
//! lookup `$HOME` and `~` use (LESSONS: "The tilde gate is mask-driven; the
//! `HOME` lookup is state-driven") — component normalisation (D-619-7: the
//! path is rebuilt from its non-empty components, exactly bash's rendering of
//! trailing and double separators), and the value parse (D-619-6): strict
//! ASCII decimal with an optional `+` — `parse_base0_int` is *not* the right
//! helper here, because bash's `010` is decimal and its `0x8` and `1e3` are
//! rejected, measured. The basename, the full rendering and the
//! `PROMPT_DIRTRIM` ellipsis at bash's **two** measured thresholds (D-619-5:
//! the plain branch trims at `components > n`, the `~` branch only at
//! `rest > n + 1`) live in `render.rs`; a relative origin displays as typed
//! (D-619-3) and the dirstack `+`/`-` prefix never appears (D-619-8, no
//! dirstack).

pub(crate) mod render;

use alloc::vec::Vec;

use crate::repl::prompt::segment::ValueEscape;
use crate::repl::prompt::value::cwd::display::render::{Root, basename, full, trimmed};
use crate::state::ShellState;
use sys::{Origin, ShortCStr};

/// Answer one of the two escapes for the state's `%CWD` fd; `None` is decision
/// D2's "no value" — an origin that names no path (D-619-2), or a failed
/// borrow. `\W` is the basename of the *abbreviated* display path (bash
/// measured `[~][~]` at `$HOME`), and `PROMPT_DIRTRIM` cannot move it: a
/// single component never passes either threshold (measured unchanged).
pub(crate) fn answer(state: &ShellState, esc: ValueEscape) -> Option<Vec<u8>> {
    let var = state.fd_var(&c"CWD".into())?;
    let home = state.var_value(&c"HOME".into());
    let path = origin_path(&var.trace.origin, state)?;
    let (root, components) = split(path.as_bytes().ok()?, home);
    if esc == ValueEscape::ShortWorkingDir {
        return Some(basename(&root, &components));
    }
    Some(
        match state
            .var_value(&c"PROMPT_DIRTRIM".into())
            .and_then(parsed_dirtrim)
        {
            Some(n) if components.len() > threshold(&root, n) => {
                trimmed(&root, n as usize, &components)
            }
            _ => full(&root, &components),
        },
    )
}

/// The path an origin names (D-619-1): a `File` path as recorded (relative
/// stays relative, D-619-3) and `EnvVar(HOME)` through the shell's `HOME`
/// lookup (so `cd ~/x` draws `~/x`, bash-identical). Every other origin names
/// no path and the escape answers `None` (D-619-2).
fn origin_path<'s>(origin: &'s Origin, state: &'s ShellState) -> Option<&'s ShortCStr> {
    match origin {
        Origin::File(path) => Some(path),
        Origin::EnvVar(name) if name.eq_bytes(b"HOME") => state.var_value(name),
        _ => None,
    }
}

/// The `~` branch's remainder: `Some` when the path **is** `$HOME` (bash draws
/// `~` alone there — measured `[~][~]`) or starts with `$HOME/`. D-619-4
/// divergences pinned in `cwd/tests.rs`: after `unset HOME` bash drops the
/// abbreviation while fdshell still sees `HOME` through the startup snapshot.
fn tilde_rest<'a>(path: &'a [u8], home: Option<&ShortCStr>) -> Option<&'a [u8]> {
    let home = home?.as_bytes().ok()?;
    if path == home {
        return Some(b"");
    }
    path.strip_prefix(home)?.strip_prefix(b"/")
}

/// Root and normalised components (D-619-7): the non-empty `/`-separated
/// segments of the displayed path, so the rule's component count and the
/// rendering never disagree.
fn split<'a>(path: &'a [u8], home: Option<&ShortCStr>) -> (Root, Vec<&'a [u8]>) {
    let (root, body) = match tilde_rest(path, home) {
        Some(rest) => (Root::Tilde, rest),
        None if path.starts_with(b"/") => (Root::Absolute, path),
        None => (Root::Relative, path),
    };
    (
        root,
        body.split(|byte| *byte == b'/')
            .filter(|part| !part.is_empty())
            .collect(),
    )
}

/// The two measured thresholds (D-619-5): the plain branch trims at
/// `components > n`, the `~` branch only at `rest > n + 1` — at least two
/// components must be droppable. A single-rule "simplification" changes
/// behaviour; the boundary rows in `cwd/tests.rs` pin both.
fn threshold(root: &Root, n: u32) -> usize {
    match root {
        Root::Tilde => n.saturating_add(1) as usize,
        _ => n as usize,
    }
}

/// D-619-6: strict ASCII decimal with an optional `+`, the whole value —
/// `u32`'s own `FromStr` is exactly that, so `010` is decimal 10 and `0x8`,
/// `1e3`, `-1` and surrounding whitespace are rejected (all measured; the
/// whitespace rows are pinned divergences: bash tolerates it). Unparseable,
/// `0` and overflow answer `None` = no trim (the `0` row answers no trim by
/// the manual's `> 0` rule, confirmed by the measured `-1` row).
fn parsed_dirtrim(value: &ShortCStr) -> Option<u32> {
    let digits = core::str::from_utf8(value.as_bytes().ok()?).ok()?;
    digits.parse::<u32>().ok().filter(|n| *n > 0)
}
