// mutants: -C --lib -C --test -C colon -C --test -C unset_expand -C --test -C positional -C --test -C arith -C --test -C quotes -C --test -C cmd_subst -C --test -C redirect -- -E 'test(/^substitute::/) | test(/^command_word::/) | binary(colon) | binary(unset_expand) | binary(positional) | binary(arith) | binary(quotes) | binary(cmd_subst) | binary(redirect)'
//! Which arms are **live** on the surface that owns the word, decided once at
//! that surface's entry point and threaded down to the arm — one enum per arm,
//! the policy supplied as data (task #371; the `~` arm's sibling is task #620's).
//! The gates that read the policy, and the mask gate beside them, live here too.

use crate::mask::MaskBit;

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

/// A word is fully quoted when every byte was consumed inside double quotes,
/// or when the word is empty — an empty mask only arises from quoted
/// material (a quoted empty word like `""`, or a quoted expansion that
/// produced nothing), which is one word even when empty.
pub(crate) fn fully_quoted(mask: &[MaskBit]) -> bool {
    mask.is_empty() || mask.iter().copied().all(MaskBit::quoted)
}

/// A word whose bytes can start an **expansion**: `$`, a backtick, a `%` of a
/// live arm, or a leading `~`. This is a **raw-text** test, never a mask test:
/// `"$(echo ls)"` carries mask `true` on every byte and must still expand
/// (bash doubles quotes do not suppress command substitution), so a
/// mask-gated word is an unsound false negative. A false positive is harmless
/// — every arm of the pipeline is correct on a word with no trigger (and on a
/// quoted `%`, which the arm's own mask-bit gate leaves literal, task #252).
/// `percent` is the surface's arm policy (task #371): a word enters
/// substitution when it carries a trigger byte of an arm that is **live** on
/// this surface. On the argument surface the `%` arm fires, so the word
/// pipeline owns the `%name` lookup. On the **redirect target** it is off —
/// measured in bash 5.3.9, no `%` form expands in a redirection word
/// (`> a%b` → the file `a%b`, `> a%CWD/x` → `ENOENT` for that literal name),
/// and fdshell decides every `%fd` redirect target at **parse** time — the
/// attached `>%name` is a `RedirectSource::Var`, the spaced `%name` is
/// `invalid redirect syntax` — so a `%` byte in a path word is file-name data.
/// Keeping `%` out of the gate keeps `> a%b` and `> a%CWD/x` at bash's answer
/// and keeps the no-trigger fast path for `%`-only words; a word that carries
/// *another* trigger reaches the substitution with the arm itself off, so
/// `> a%log$(echo x)` writes the literal `a%logx` exactly as bash does.
pub(crate) fn has_trigger(bytes: &[u8], percent: PercentArm) -> bool {
    leads_tilde(bytes)
        || bytes
            .iter()
            .any(|b| matches!(b, b'$' | b'`') || (*b == b'%' && percent == PercentArm::Fires))
}

/// The tilde trigger is positional: only a **leading** `~` can expand
/// (POSIX #2.6.1, and `substitute/tilde.rs` is the arm).
fn leads_tilde(bytes: &[u8]) -> bool {
    bytes.first() == Some(&b'~')
}
