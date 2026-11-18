// mutants: -C --lib -C --test -C repl -- -E 'test(/^repl::/) | binary(repl)'

//! The rendering side of task #619's rules (the split is STYLE §2.2's, not a
//! separate contract): the root the displayed path has, the full rendering,
//! the `PROMPT_DIRTRIM` ellipsis rendering, and the basename for `\W`. Both
//! thresholds and every rule here are measured against bash 5.3.9 on
//! 2026-11-18; the measurements live in `display.rs`'s doc and the pins in
//! `cwd/tests.rs`.

use alloc::{vec, vec::Vec};

/// Which root the displayed path has: the `~` branch, an absolute origin, or
/// a relative one (D-619-3 keeps a relative origin relative).
pub(crate) enum Root {
    Tilde,
    Absolute,
    Relative,
}

/// The full rendering: root marker then the `/`-joined components. The empty
/// roots print as bash measures them (`[/][/]` at `/`, `[~][~]` at `$HOME`);
/// a relative root prints nothing.
pub(crate) fn full(root: &Root, components: &[&[u8]]) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::new();
    match root {
        // The `~` root is itself one component's position: it prints, and
        // every component behind it takes a separator.
        Root::Tilde => out.extend_from_slice(b"~"),
        // The root `/` glues to the first component (bash's rendering); an
        // empty component list leaves it alone, which is bash's `[/][/]`.
        Root::Absolute => out.push(b'/'),
        Root::Relative => {}
    }
    let leading = matches!(root, Root::Tilde);
    for (i, component) in components.iter().enumerate() {
        if leading || i > 0 {
            out.push(b'/');
        }
        out.extend_from_slice(component);
    }
    out
}

/// The ellipsis rendering: `...` replaces the leading components and the last
/// `n` are kept (measured `n=1 → .../8` on the plain branch and `~/.../3`
/// after `~`). The `~` keeps its root; the plain root is one of the replaced
/// leading components (measured `.../8`, no leading `/`).
pub(crate) fn trimmed(root: &Root, n: usize, components: &[&[u8]]) -> Vec<u8> {
    let mut parts: Vec<&[u8]> = vec![b"..."];
    parts.extend_from_slice(components.get(components.len() - n..).unwrap_or_default());
    match root {
        Root::Tilde => full(&Root::Tilde, &parts),
        _ => full(&Root::Relative, &parts),
    }
}

/// `\W`: the last component, or the root itself when the path has none
/// (measured `[~][~]` at `$HOME` and `[/][/]` at `/`).
pub(crate) fn basename(root: &Root, components: &[&[u8]]) -> Vec<u8> {
    match components.last() {
        Some(component) => component.to_vec(),
        None => full(root, &[]),
    }
}
