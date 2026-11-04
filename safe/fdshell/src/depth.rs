//! The shared recursion cap for **data-driven** descents: arithmetic eval/parse,
//! `test` expressions and brace expansion. Each of those recurses once per
//! nesting level of its input, so its depth is a property of the data, not of
//! the script; without a cap a deep input aborts the shell with a stack
//! overflow. `nest.rs` caps the shell's own block/`$(…)` layer on
//! `ShellState`; these descents have no state to ride, so they share this one
//! counter and report a clean `Report`.

use error_stack::{Report, bail};

/// Recursion cap for data-driven descents. It must stay above every legitimate
/// pinned depth (100 — `nest.rs`, `arith/var.rs`) and below the lowest measured
/// crash depth (2 200, brace expansion).
pub(crate) const MAX_DATA_DEPTH: u32 = 1_000;

/// Run `f` at `depth`, or report `limit` when `depth` is past
/// [`MAX_DATA_DEPTH`]. Each recursive descent calls it once per level, at its
/// own entry, and passes `depth + 1` down the recursion.
pub(crate) fn guarded<E, T, F>(depth: u32, limit: E, f: F) -> Result<T, Report<E>>
where
    E: core::error::Error + Send + Sync + 'static,
    F: FnOnce() -> Result<T, Report<E>>,
{
    if depth > MAX_DATA_DEPTH {
        bail!(limit);
    }
    f()
}

#[cfg(test)]
mod tests;
