//! The redirect target word **with** an expansion trigger: the word pipeline,
//! then the redirect's own glob policy, then bash's one-name rule (task #278).

use alloc::vec::Vec;
use error_stack::{Report, ResultExt};
use hashbrown::HashMap;
use sys::ExportedFd;
use sys::LocalFd;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

use crate::error::redirect::OpenRedirectError;
use crate::state::ShellState;

use super::{ban_absolute, one_name};

/// Expand the target word to its fields and glob every field at the
/// redirect's own `base`, then apply the one-name rule.
///
/// The two deltas from an argument word's pipeline, and the reason the target
/// calls [`word_fields`] instead of `substitute_args`: the glob is rooted at
/// the strict `%CWD` fd (`glob::matches(…, base)`, the base `target.rs` is
/// handed) and not at the process CWD, and a pattern with **no** match keeps
/// its literal word — `nullglob` and `failglob` do not apply to a redirect
/// target (task #369 owns that decision, measured against bash). So
/// `> $(true)` yields 0 names and `n="a b"; > $n` yields 2, both
/// `ambiguous redirect` as bash reports, while `> zzz*` still creates `zzz*`.
pub(super) fn target(
    path: &ShortCStr,
    mask: &[bool],
    spans: &[usize],
    quoted: bool,
    base: Option<&LocalFd>,
    cache: &mut HashMap<ShortCStr, ExportedFd>,
    cell: &ForkCell<ShellState>,
) -> Result<ShortCStr, Report<OpenRedirectError>> {
    let fields = crate::substitute::word_fields(path, mask, spans, quoted, cache, cell)
        .change_context(OpenRedirectError::TargetExpand)?;
    let mut names: Vec<ShortCStr> = Vec::new();
    for (field, field_mask) in &fields {
        // The ban is evaluated on the **expansion**, before the walk: strict
        // mode exists to close `> $(echo /etc/x)`, not just `> /etc/x`.
        ban_absolute(field, base)?;
        let mut matches = crate::glob::matches(field, field_mask, cell, base)
            .change_context(OpenRedirectError::Glob)?;
        if matches.is_empty() {
            matches.push(field.clone());
        }
        names.extend(matches);
    }
    one_name(&names, path)
}
