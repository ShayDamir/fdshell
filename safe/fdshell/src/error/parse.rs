//! Parser errors (parse/*.rs).
//!
//! `ParseError` covers parse-time errors without position info.
//! Position is attached separately via `ParsePosition` on the `Report`.

use error_stack::Report;

/// [ParseError] Parser errors
// Debug needed for impl Error (trait bound), not for display.
#[derive(displaydoc::Display, Debug)]
pub(crate) enum ParseError {
    /// unmatched quote
    UnbalancedQuote,
    /// unexpected end of input
    UnexpectedEof,
    /// invalid character
    InvalidChar { ch: u8 },
    /// case: missing 'in'
    CaseMissingIn,
    /// case: empty pattern
    CaseEmptyPattern,
    /// case: missing 'esac'
    CaseMissingEsac,
    /// case: missing ')'
    CaseMissingCloseParen,
    /// internal invariant violated
    Never,
    /// duplicate redirect target
    DuplicateRedirect,
    /// not a valid octal number
    InvalidOctal,
    /// unexpected character in this context
    UnexpectedChar { ch: u8 },
    /// invalid redirect syntax
    InvalidRedirect,
    /// break takes no arguments
    BreakTakesNoArguments,
    /// continue takes no arguments
    ContinueTakesNoArguments,
    /// expected ';' before 'done'
    ExpectedSemicolonBeforeDone,
    /// expected command
    ExpectedCommand,
    /// expected command after pipe
    ExpectedCommandAfterPipe,
    /// expected condition
    ExpectedCondition,
    /// expected 'do'
    ExpectedDo,
    /// expected 'done'
    ExpectedDone,
    /// expected 'for'
    ExpectedFor,
    /// expected 'in'
    ExpectedIn,
    /// expected variable name
    ExpectedVariableName,
    /// expected variable name after 'unset'
    ExpectedVariableNameAfterUnset,
    /// expected word list
    ExpectedWordList,
    /// malformed case block
    MalformedCaseBlock,
    /// malformed if block
    MalformedIfBlock,
    /// missing condition
    MissingCondition,
    /// missing 'else' body
    MissingElseBody,
    /// missing 'fi'
    MissingFi,
    /// missing 'then'
    MissingThen,
    /// missing 'then' after 'elif'
    MissingThenAfterElif,
    /// umask takes at most one argument
    UmaskTakesAtMostOneArgument,
    /// unexpected pipe
    UnexpectedPipe,
    /// variable must start with '%'
    VariableMustStartWithPercent,
    /// capture syntax missing '%' before variable name
    CaptureMissingPercent,
    /// capture syntax has no variable name after '%'
    CaptureEmptyVar,
}

impl std::error::Error for ParseError {}

/// Create a `Report<ParseError>` for an unbalanced quote at `pos`.
pub(crate) fn report_unbalanced_quote(line: &[u8], pos: usize) -> Report<ParseError> {
    Report::new(ParseError::UnbalancedQuote).attach_opaque(ParsePosition {
        pos,
        input: Some(line.to_vec()),
    })
}

/// Create a `Report<ParseError>` for unexpected EOF starting at `pos`.
pub(crate) fn report_unexpected_eof(line: &[u8], pos: usize) -> Report<ParseError> {
    Report::new(ParseError::UnexpectedEof).attach_opaque(ParsePosition {
        pos,
        input: Some(line.to_vec()),
    })
}

/// Create a `Report<ParseError>` for an invalid character at `pos`.
pub(crate) fn report_invalid_char(ch: u8, pos: usize) -> Report<ParseError> {
    Report::new(ParseError::InvalidChar { ch }).attach_opaque(ParsePosition { pos, input: None })
}

/// Attached with the parse error to show position in error output.
#[derive(Debug)]
pub(crate) struct ParsePosition {
    /// Byte offset of the error in the input.
    pub(crate) pos: usize,
    /// The input line for formatting.
    pub(crate) input: Option<Vec<u8>>,
}
