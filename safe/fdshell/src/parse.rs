// mutants: -C --lib -- -E 'test(/^parse::/)'

mod arith_cmd;
mod array_literal;
pub(crate) mod array_ref;
mod backtick;
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
mod token_word;
pub(crate) mod umask_mode;
pub(crate) mod wait_block;
pub(crate) mod while_block;
pub(crate) mod word_quoted;

pub(crate) use array_literal::{ArrayAssign, ArrayWord};
pub(crate) use case_block::literal_indices;
pub use cmdline::stage::{BlockStage, ExpandedStage, Pipeline, Stage};
pub use cmdline::{BuiltinPrefix, CommandLine};
pub use envassign::{Assign, value_mask, value_spans};
pub(crate) use here_string::word_indices;
pub(crate) use heredoc::delimiter_token_indices;
pub use line::{ParsedLine, UnsetTarget};
pub(crate) use redirect::operand_offset;
pub(crate) use redirect::parse_fd;
pub(crate) use redirect::{clobber_word, pipe_token_span, pipeline_pipe};
pub(crate) use semi::pair_at_byte;
pub(crate) use time_block::TimeBlock;
pub(crate) use token_word::Token;

use crate::error::parse::ParseError;
use error_stack::{Report, ResultExt};
use sys::ScriptText;

pub(crate) fn parse(text: &ScriptText) -> Result<ParsedLine, Report<ParseError>> {
    let line = text.as_bytes().change_context(ParseError::Never)?;
    stmt::parse_statement(text, line)
}

#[cfg(test)]
mod tests;
