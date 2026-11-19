// mutants: -C --lib -C --test -C case -C --test -C trap -- -E 'test(/^case_exec::/) | binary(case) | binary(trap)'
//! One `case` clause's match test: the pattern word, then the glob match,
//! clause by clause, in the order POSIX 2.9.4.3 asks them (task #319).

use crate::glob::match_component;
use crate::parse::case_clause::CaseClause;
use crate::substitute::pattern::word as pattern_word;
use error_stack::{Report, ResultExt};
use hashbrown::HashMap;

use crate::error::cmd::CmdError;
use crate::state::ShellState;
use sys::ExportedFd;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

/// Does `clause` match `word_bytes`? The patterns are asked in order, one at a
/// time, and the first match decides — bash's laziness, pinned by the
/// `$(touch f)` pins.
///
/// POSIX 2.9.4.3 expands a pattern word (tilde, parameter and command
/// substitution, quote removal) **with the pattern's own mask**, and bash
/// expands it lazily: pattern by pattern, stopping at the first match, so
/// `case a in a|$(touch f))` never runs the substitution. The pattern is a
/// single **word**: bash keeps only the first expanded word of it
/// (`execute_case_command`, `es->word->word`), splits it on nothing and globs
/// it never, so `substitute/pattern.rs::word` cuts the expanded text only on
/// the `$@` word boundaries and keeps the first piece — a quoted `"$@"` in a
/// pattern is the **first positional**, not a pattern per positional (task
/// #319). The mask the expansion returns is bash's pattern-quoting signal: an
/// expansion's bytes inherit the trigger's bit, so `"$p"` keeps an expanded `?`
/// literal and `$p` lets it glob. A word pattern has no FNM_PERIOD rule
/// (LESSONS): dotglob = true.
pub(super) fn matches(
    clause: &CaseClause,
    word_bytes: &[u8],
    cache: &mut HashMap<ShortCStr, ExportedFd>,
    cell: &ForkCell<ShellState>,
) -> Result<bool, Report<CmdError>> {
    for pattern in &clause.patterns {
        let (pat, pat_mask) =
            pattern_word(&pattern.text, &pattern.mask, &pattern.spans, cache, cell)
                .change_context(CmdError::Resolve)?;
        let pattern_bytes = pat.as_bytes().change_context(CmdError::Never)?;
        if match_component(pattern_bytes, &pat_mask, word_bytes, true) {
            return Ok(true);
        }
    }
    Ok(false)
}
