//! Whether the `%name` fd arm is **live** on the surface that owns the word,
//! decided once at that surface's entry point and threaded down to the arm —
//! the sibling of `Join`, supplied as data (task #371).

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
