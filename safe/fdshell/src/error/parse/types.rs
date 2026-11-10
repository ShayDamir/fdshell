//! The `ParseError` enum; the facade (`error/parse.rs`) re-exports it and
//! implements `Error`.

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
    /// case: missing ';;' between clauses
    CaseMissingSemiSemi,
    /// case: empty word
    CaseEmptyWord,
    /// case: ';' before an empty clause list
    CaseEmptyClauseList,
    /// case: '(' and ')' are control operators here (quote them)
    CaseWordOperator,
    /// case arm: unexpected ')' — a command list cannot take a paren
    CaseArmUnexpectedCloseParen,
    /// stray ';;' — it only separates 'case' clauses
    UnexpectedSemiSemi,
    /// array assignment is missing its closing ')'
    ArrayLiteralMissingCloseParen,
    /// internal invariant violated
    Never,
    /// not a valid octal number
    InvalidOctal,
    /// unexpected character in this context
    UnexpectedChar { ch: u8 },
    /// invalid redirect syntax
    InvalidRedirect,
    /// ambiguous redirect: {name}
    AmbiguousRedirect { name: sys::ShortCStr },
    /// brace expansion produced too many words
    BraceExpansionTooManyWords,
    /// brace expansion nested too deeply; reduce the nesting of brace groups
    BraceExpansionTooDeep,
    /// break/continue takes at most one argument (a positive loop count)
    BreakContinueTakesAtMostOneArgument,
    /// break/continue count must be a positive integer
    LoopCountInvalid,
    /// return takes at most one argument
    ReturnTakesAtMostOneArgument,
    /// 'return' status must be an integer
    InvalidInt,
    /// function definition has an empty name
    FunctionEmptyName,
    /// function definition is missing a closing brace
    FunctionMissingCloseBrace,
    /// function definition body is empty
    FunctionEmptyBody,
    /// '{word}' is not a valid function name (a command substitution is not
    /// part of a definer word)
    FunctionNameWithSubstitution { word: sys::ShortCStr },
    /// '{word}' cannot be written between the function definer and its '{{'
    /// (a redirection belongs after the closing brace)
    FunctionRedirectBeforeBrace { word: sys::ShortCStr },
    /// group is missing a closing brace
    GroupMissingCloseBrace,
    /// group is empty
    GroupEmpty,
    /// expected ';' before '}}'
    ExpectedSemicolonBeforeCloseBrace,
    /// group: unexpected word after '}}'
    GroupTrailingWord,
    /// group: here-doc on a group is not supported
    GroupHereDocNotSupported,
    /// expected ';' before 'done'
    ExpectedSemicolonBeforeDone,
    /// expected ';' before 'fi'
    ExpectedSemicolonBeforeFi,
    /// 'then' body is empty
    EmptyThenBody,
    /// 'elif' body is empty
    EmptyElifBody,
    /// 'else' body is empty
    EmptyElseBody,
    /// 'do' body is empty
    EmptyDoBody,
    /// expected command
    ExpectedCommand,
    /// expected command after pipe
    ExpectedCommandAfterPipe,
    /// expected command after '{op}'
    ExpectedCommandAfterOp { op: sys::ShortCStr },
    /// expected command before '{op}'
    ExpectedCommandBeforeOp { op: sys::ShortCStr },
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
    /// expected word list
    ExpectedWordList,
    /// malformed case block
    MalformedCaseBlock,
    /// malformed group block
    MalformedGroupBlock,
    /// malformed if block
    MalformedIfBlock,
    /// missing condition
    MissingCondition,
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
    /// capture syntax missing '%' before variable name
    CaptureMissingPercent,
    /// capture syntax has no variable name after '%'
    CaptureEmptyVar,
    /// malformed `wait` block
    MalformedWaitBlock,
    /// `wait`: empty block (no arms)
    WaitEmptyBlock,
    /// `wait`: empty arm pattern
    WaitEmptyPattern,
    /// `wait`: missing ')' after arm pattern
    WaitMissingCloseParen,
    /// `wait`: unknown arm pattern keyword
    WaitUnknownPattern,
    /// `wait`: `readable`/`writable`/`finished` need an fd reference
    WaitMissingFd,
    /// `wait`: fd reference must start with '%'
    WaitFdRefPercent,
    /// `wait`: `after` needs a millisecond deadline
    WaitMissingTimeout,
    /// `wait`: `after` deadline must be a non-negative integer
    WaitInvalidTimeout,
    /// `wait`: unexpected token in arm pattern
    WaitUnexpectedToken,
    /// here-doc/here-string: '{prefix}' is not a valid descriptor number (a decimal fd
    /// number in `0 ..= 2147483647`, as in `N>file`)
    InvalidRedirFdWord { prefix: sys::ShortCStr },
    /// arithmetic command must be `((expr))` with no trailing words
    MalformedArithCommand,
    /// `for ((expr;expr;expr))` needs a closing `))`
    ArithForMissingCloseParen,
    /// `for ((expr;expr;expr))` needs exactly two `;` separators
    ArithForClauseCount,
    /// '{opener}' is missing its closing '{closer}'
    MissingBlockCloser {
        opener: sys::ShortCStr,
        closer: sys::ShortCStr,
    },
    /// '{closer}' must be followed by a separator (';', a newline, '&', '|') —
    /// unexpected word
    BlockTrailingWord { closer: sys::ShortCStr },
    /// stray '{closer}' — it closes no open block
    StrayCloser { closer: sys::ShortCStr },
}
