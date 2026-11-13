// mutants: -C --lib -- -E 'test(/^parse::case_block::/)'

mod literal;

pub(crate) use literal::{literal_indices, owns_semi, up_to_esac};

use super::Token;
use super::case_clause;
use super::semi::{semi_byte, trim_semi, try_join_masked};
use super::word_quoted::{keyword_word, slot_rejects};
use crate::error::parse::ParseError;
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

    // POSIX 2.9.4.3 spells `case WORD in` and POSIX 2.9.1 puts control operators
    // at this position, so bash 5.3.9 rejects a word-opening `(` and a bare `)`
    // here (task #470); the gate is the shared [`slot_rejects`], asked of every
    // token of the trimmed slot **before** the join and the clause walk, because
    // bash reports the earliest offending token (the `for` site documents the same
    // order). The slot's owned set is only the `;` that [`trim_semi`] owns — the
    // separator this position owns as its own (pinned: `case x; in esac` parses);
    // every other row of the ONE set is rejected with it (tasks #495/#496's rule:
    // each site that listed only the words it happened to notice is one bug).
    if word_tokens
        .iter()
        .any(|tok| slot_rejects(tok, line, &owns_semi))
    {
        bail!(ParseError::CaseWordOperator);
    }

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

#[cfg(test)]
mod tests;
