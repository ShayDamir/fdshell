//! The **pipeline-stage** arm of the command-position rule (task #292): the
//! word that follows a `|` is in command position, so a keyword there opens a
//! block which is a whole stage — `echo a | while :; do echo b; break; done`.
//! The rule is the fourth arm of the classification the run tests express
//! ([`super::group_at_run`] carries the other three: run start, cond part
//! mid-run, argument), and it is deliberately *not* folded into that function:
//! its answer drives a segment flush, and a stage's text must stay inside the
//! run so the pipeline parser can split the stages.
//!
//! The `{ …; }` group keeps its word at this position (tasks #186/#190:
//! `echo a | { echo b; }` is a parse error): the brace scorer is
//! `cond/boundary::opens_group`, which asks [`super::group_at_run`] and
//! [`super::definer_at_run`] only, never this rule.

use crate::parse::pipeline_pipe;

use super::group_at_run;

/// The word at `word` opens a **pipeline stage** of the run that starts at
/// `start`: the run text before it ends with the tokenizer's pipe byte. The
/// byte test is [`pipeline_pipe`] — the ONE pipeline-pipe rule, the tokenizer's
/// pipe token on raw bytes — so a quoted `"|"`, an escaped `\|` and the `|`
/// absorbed into a `>`-terminated operator word (`>|`, `&>|`, `%>|`) stay word
/// data, exactly as the token layer reads them (LESSONS: the byte-level and
/// token-level rules must agree). A `|` whose twin makes the pair a cond
/// operator (`||`) is the cond-part rule's own (`segment/block/midrun.rs`).
pub(crate) fn stage_at_run(line: &[u8], start: usize, word: usize) -> bool {
    let run = line.get(start..word).unwrap_or(b"").trim_ascii_end();
    match run.len().checked_sub(1) {
        Some(k) if run.get(k) == Some(&b'|') => {
            let pipe = start + k;
            let alone = pipe
                .checked_sub(1)
                .is_none_or(|p| line.get(p) != Some(&b'|'));
            alone && pipeline_pipe(line, pipe)
        }
        _ => false,
    }
}

/// A word in **command position** on the run that starts at `start`: a run
/// start, a cond part, or a pipeline stage. This is the scorer the cond layer
/// asks (one call, one rule for all three arms); the `{`-brace exclusion stays
/// in `cond/boundary::opens_group`, which keeps asking [`group_at_run`].
pub(crate) fn command_at_run(line: &[u8], start: usize, word: usize) -> bool {
    group_at_run(line, start, word).is_some() || stage_at_run(line, start, word)
}

#[cfg(test)]
mod tests;
