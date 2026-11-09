// mutants: -C --lib -- -E 'test(/^parse::case_block::/)'

mod literal;

pub(crate) use literal::literal_indices;

use super::Token;
use super::case_clause;
use super::semi::{List, list_closing_keyword_index, semi_byte, trim_semi, try_join_masked};
use super::word_quoted::keyword_word;
use crate::error::parse::ParseError;
use crate::keywords::Closer;
use crate::mask::MaskBit;
use crate::redirect::RedirectDef;
use alloc::vec::Vec;
use error_stack::{Report, ResultExt, bail, ensure};
use sys::ScriptText;
use sys::ShortCStr;

pub struct CaseBlock {
    pub word: ShortCStr,
    /// The per-byte quote mask of `word`, parallel to it: the joined word
    /// tokens' masks with a `false` bit per inserted space (task #269), so the
    /// `case` word is expanded with its own quoting.
    pub word_mask: Vec<MaskBit>,
    /// Zero-byte quote-span indices of the joined word (task #321), shifted by
    /// the join offsets like the mask.
    pub word_span: Vec<usize>,
    pub clauses: Vec<case_clause::CaseClause>,
    /// The redirections written after the closing `esac` (task #298,
    /// `parse::block_tail`); they apply to the whole `case`, and open **before**
    /// the `case` word is expanded (bash's order, measured).
    pub redirects: Vec<RedirectDef>,
}

pub(crate) fn tokens_to_case(
    tokens: &[Token],
    text: &ScriptText,
) -> Result<CaseBlock, Report<ParseError>> {
    ensure!(
        tokens
            .first()
            .is_some_and(|Token { word: t, .. }| t.eq_bytes(b"case")),
        ParseError::MalformedCaseBlock
    );

    let line = text.as_bytes().change_context(ParseError::Never)?;
    let in_idx = (1..tokens.len())
        .find(|&i| {
            tokens
                .get(i)
                .is_some_and(|tok| keyword_word(tok, line, b"in"))
        })
        .ok_or(ParseError::CaseMissingIn)?;

    // A clause-position heredoc's bodies sit in the block text after the
    // closing `esac`: the tokens after it are body data, not block structure
    // (task #273, [`up_to_esac`]) — the clause list is walked as a `case` word
    // list, so a closer-shaped **pattern** (`case x in done) …`) stays word
    // data. The `in` index is a prefix index, so the truncation (which cuts a
    // suffix) leaves it valid.
    let closed = up_to_esac(tokens, line)?;
    // The closing `esac` is the truncated slice's last token; the tail it leaves
    // over the block text is the block's redirection (task #298). In the lenient
    // branch of [`up_to_esac`] the closer *is* the last token, so the tail is
    // empty there, exactly as bash reads it.
    let esac_idx = closed.len() - 1;
    let redirects =
        crate::parse::block_tail::redirects(b"esac", tokens.get(esac_idx..).unwrap_or(&[]), text)?;

    // POSIX #2.9.1's `case_clause` production requires a `word` between `case`
    // and `in`, so an all-separator (or empty) word slot has none — bash rejects
    // every spelling of it (`case;`, `case ; ;`, `case` + newline, a comment-only
    // word) at rc 2, fdshell at its own rc 1 (task #415). The test is on the
    // **trimmed** slice (LESSONS: "An empty block arm body is a parser rule"); a
    // quoted empty pair or a quoted `";"` is a **token**, so those words stay.
    let word_tokens = trim_semi(closed.get(1..in_idx).ok_or(ParseError::CaseMissingIn)?);
    ensure!(!word_tokens.is_empty(), ParseError::CaseEmptyWord);

    let (word, word_mask, word_span) = try_join_masked(word_tokens);

    let clauses = case_clause::parse_clauses(closed, in_idx + 1, esac_idx, text)?;
    // The slot after `in` may stay empty (`case x in esac`, bash rc 0), but POSIX
    // spells it a `newline_list`: a real `;` **byte** in it is the grammar
    // violation while the newline that tokenizes as a `;` token is not
    // (task #415, LESSONS: "A `;` token is a newline in disguise"). Only the
    // **empty** slot is guarded — the walk's own verdicts stay in charge of every
    // slot it turned into a clause, and the leading-`;`-before-a-clause leniency
    // (task #300) stays as measured.
    if clauses.is_empty() {
        // The `+ 1` operand is dead — the guard below scans the slot for a real
        // `;` **byte** and the `in` token is never one — so it is written as a
        // method call, which leaves no operator site (CATCHING_MUTANTS rules 22/24).
        let slot_from = in_idx.saturating_add(1);
        let slot = closed.get(slot_from..esac_idx).unwrap_or(&[]);
        if slot.iter().any(|tok| semi_byte(tok, line)) {
            bail!(ParseError::CaseEmptyClauseList);
        }
    }

    Ok(CaseBlock {
        word,
        word_mask,
        word_span,
        clauses,
        redirects,
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
    let rest = match list_closing_keyword_index(tokens, line, Closer::Esac, List::Case) {
        Some(idx) => tokens.get(..=idx).ok_or(ParseError::CaseMissingEsac)?,
        None => tokens,
    };
    ensure!(
        rest.last()
            .is_some_and(|tok| crate::parse::block_tail::is_closer(tok, b"esac")),
        ParseError::CaseMissingEsac
    );
    Ok(rest)
}

#[cfg(test)]
mod tests;
