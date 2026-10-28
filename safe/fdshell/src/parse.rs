mod arith_cmd;
mod array_ref;
mod backtick;
use alloc::vec::Vec;
mod bg_redirect;
pub(crate) mod block_tail;
mod builtin;
mod capture;
pub(crate) mod case_block;
pub(crate) mod case_clause;
mod classify;
mod cmdline;
mod command;
mod command_args;
mod comment;
mod cond_bodies;
mod detect;
mod detect_keyword;
mod dispatch;
mod elif;
mod emit;
mod envassign;
mod fd_dup;
pub(crate) mod for_block;
pub(crate) mod function_block;
pub(crate) mod group_block;
mod here_string;
mod heredoc;
pub(crate) mod if_block;
mod line;
mod part_text;
mod pipeline;
mod quotes;
mod redirect;
mod redirect_op;
mod semi;
mod stmt;
pub(crate) mod time_block;
pub(crate) mod token;
mod token_pipe;
mod token_subst;
pub(crate) mod wait_block;
pub(crate) mod while_block;
pub(crate) mod word_quoted;

pub(crate) use case_block::literal_indices;
pub use cmdline::{BlockStage, BuiltinPrefix, CommandLine, ExpandedStage, Pipeline, Stage};
pub use envassign::{Assign, value_mask, value_spans};
pub(crate) use here_string::word_indices;
pub(crate) use heredoc::delimiter_token_indices;
pub use line::{ParsedLine, UnsetTarget};
pub(crate) use redirect::operand_offset;
pub(crate) use redirect::parse_fd;
pub(crate) use redirect::{clobber_word, pipe_token_span, pipeline_pipe};
pub(crate) use time_block::TimeBlock;

use crate::error::parse::ParseError;
use crate::mask::MaskBit;
use error_stack::{Report, ResultExt};
use sys::ScriptText;
use sys::ShortCStr;

/// A token: its unquoted word (empty for a quoted empty word such as `""`),
/// the byte range `(start, end)` of its raw text, whether it was fully
/// quoted, the per-byte quote mask (parallel to the word; `true` marks
/// bytes that were inside quotes (double **or** single) and are protected from
/// IFS word splitting), and the word-text indices at which a quote span closed
/// **without contributing any byte** (task #321: the mask cannot see a `""` /
/// `''` pair — zero bytes, zero bits — so the tilde rule consults this list;
/// `~""/x` → text `~/x`, mask all-`false`, spans `[1]`). Parallel plumbing:
/// every surface that carries a mask carries this list at the same offset.
pub(crate) type Token = (ShortCStr, usize, usize, bool, Vec<MaskBit>, Vec<usize>);

pub(crate) fn parse(text: &ScriptText) -> Result<ParsedLine, Report<ParseError>> {
    let line = text.as_bytes().change_context(ParseError::Never)?;
    stmt::parse_statement(text, line)
}

#[cfg(test)]
mod tests;
