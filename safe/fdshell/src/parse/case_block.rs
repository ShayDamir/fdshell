mod literal;

pub(crate) use literal::literal_indices;

use super::Token;
use super::case_clause;
use super::semi::{List, list_closing_keyword_index, trim_semi, try_join_masked};
use crate::error::parse::ParseError;
use alloc::vec::Vec;
use error_stack::{Report, ResultExt, ensure};
use sys::ScriptText;
use sys::ShortCStr;

pub struct CaseBlock {
    pub word: ShortCStr,
    /// The per-byte quote mask of `word`, parallel to it: the joined word
    /// tokens' masks with a `false` bit per inserted space (task #269), so the
    /// `case` word is expanded with its own quoting.
    pub word_mask: Vec<bool>,
    /// Zero-byte quote-span indices of the joined word (task #321), shifted by
    /// the join offsets like the mask.
    pub word_span: Vec<usize>,
    pub clauses: Vec<case_clause::CaseClause>,
}

pub(crate) fn tokens_to_case(
    tokens: &[Token],
    text: &ScriptText,
) -> Result<CaseBlock, Report<ParseError>> {
    ensure!(
        tokens
            .first()
            .is_some_and(|(t, _, _, _, _, _)| t.eq_bytes(b"case")),
        ParseError::MalformedCaseBlock
    );

    let in_idx = (1..tokens.len())
        .find(|&i| {
            tokens
                .get(i)
                .is_some_and(|(t, _, _, _, _, _)| t.eq_bytes(b"in"))
        })
        .ok_or(ParseError::CaseMissingIn)?;

    // A clause-position heredoc's bodies sit in the block text after the
    // closing `esac`: the tokens after it are body data, not block structure
    // (task #273, [`up_to_esac`]) — the clause list is walked as a `case` word
    // list, so a closer-shaped **pattern** (`case x in done) …`) stays word
    // data. The `in` index is a prefix index, so the truncation (which cuts a
    // suffix) leaves it valid.
    let line = text.as_bytes().change_context(ParseError::Never)?;
    let tokens = up_to_esac(tokens, line)?;
    // The closing `esac` is the truncated slice's last token.
    let esac_idx = tokens.len() - 1;

    let (word, word_mask, word_span) = try_join_masked(trim_semi(
        tokens.get(1..in_idx).ok_or(ParseError::CaseMissingIn)?,
    ));

    let clauses = case_clause::parse_clauses(tokens, in_idx + 1, esac_idx, text)?;

    Ok(CaseBlock {
        word,
        word_mask,
        word_span,
        clauses,
    })
}

/// The `case` block's tokens up to and including its closing `esac`: the word
/// walk of [`list_closing_keyword_index`] with [`List::Case`], whose closer must
/// be an `esac`, plus the block's own leniency. A last clause whose body is not
/// `;;`-terminated has its `esac` in argument position, where no closer scores —
/// bash rejects the shape, fdshell accepts it
/// (`parse::tests::case_last_clause_no_semi_semi`), so an unclosed walk keeps
/// the whole list, and its last token must still be `esac`.
fn up_to_esac<'a>(tokens: &'a [Token], line: &[u8]) -> Result<&'a [Token], Report<ParseError>> {
    let rest = match list_closing_keyword_index(tokens, line, List::Case) {
        Some(idx) => tokens.get(..=idx).ok_or(ParseError::CaseMissingEsac)?,
        None => tokens,
    };
    ensure!(
        rest.last()
            .is_some_and(|(t, _, _, _, _, _)| t.eq_bytes(b"esac")),
        ParseError::CaseMissingEsac
    );
    Ok(rest)
}

#[cfg(test)]
mod tests;
