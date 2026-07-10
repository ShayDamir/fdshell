use super::case_clause;
use super::semi::{trim_semi, try_join};
use crate::error::parse::ParseError;
use error_stack::{Report, ensure};
use sys::ShortCStr;

pub struct CaseBlock {
    pub word: ShortCStr,
    pub clauses: Vec<case_clause::CaseClause>,
}

pub(crate) fn tokens_to_case(
    tokens: &[(ShortCStr, usize, bool)],
) -> Result<CaseBlock, Report<ParseError>> {
    ensure!(
        tokens.first().is_some_and(|(t, _, _)| t.eq_bytes(b"case")),
        ParseError::MalformedCaseBlock
    );

    let in_idx = (1..tokens.len())
        .find(|&i| tokens.get(i).is_some_and(|(t, _, _)| t.eq_bytes(b"in")))
        .ok_or(ParseError::CaseMissingIn)?;

    let esac_idx = tokens.len() - 1;
    ensure!(
        tokens.last().is_some_and(|(t, _, _)| t.eq_bytes(b"esac")),
        ParseError::CaseMissingEsac
    );

    let word = try_join(trim_semi(
        tokens.get(1..in_idx).ok_or(ParseError::CaseMissingIn)?,
    ))?;

    let clauses = case_clause::parse_clauses(tokens, in_idx + 1, esac_idx)?;

    Ok(CaseBlock { word, clauses })
}
