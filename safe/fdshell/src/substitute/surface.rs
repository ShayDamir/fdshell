//! Which arms are **live** on the surface that owns the word, decided once at
//! that surface's entry point and threaded down to the arm — one enum per arm,
//! the policy supplied as data (task #371; the `~` arm's sibling is task #620's).

/// The `%name` arm's policy on one surface, threaded from the entry point that
/// owns the surface down to the arm (task #371, measured against bash: bash
/// expands no `%` form anywhere, the `%fd` namespace is fdshell's own).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PercentArm {
    /// The arm fires: the word/argument surface, word 0, assignments,
    /// heredocs, `local`/`let`, `case`, patterns, `param_op` and nested
    /// command substitution. At the arm, the trigger byte's mask bit gates it
    /// (task #252): a quoted `%` is an ordinary literal byte, as bash.
    Fires,
    /// The arm is off: the **redirect target**, the one surface where a `%`
    /// byte is file-name data (task #278's gate, task #371's per-surface
    /// decision). The parse-time forms — the attached `>%name`/`&>%name` and
    /// the `>&N` duplicate word — obey their own rule and never reach the arm.
    Off,
}

/// The leading-`~` arm's policy on one surface, threaded the same way as
/// `PercentArm` (task #620, measured against bash 5.3.9 on 2026-11-18).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TildeArm {
    /// The arm fires: every word surface the tokenizer hands over — arguments,
    /// assignments, heredocs, `local`/`let`, `case`, patterns, `param_op` and
    /// nested command substitution — where a leading `~` becomes `$HOME`
    /// (POSIX #2.6.1, `substitute/tilde.rs` is the arm).
    Fires,
    /// The arm is off: the **prompt**, the one surface where a leading `~` is
    /// data (measured: `PS1='~ '` draws `~ `, not `$HOME`). The prompt's own
    /// grammar is in `repl/prompt/render/expand.rs`, the surface's only caller.
    Off,
}
