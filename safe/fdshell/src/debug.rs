//! Debug hook installation for `error_stack`.
//!
//! Registers hooks for `ParseError`, `ParsePosition`, and `Suggestion` so that
//! errors display helpful context.

use error_stack::{Report, fmt::HookContext};

use crate::error::parse::ParsePosition;
use alloc::format;
use alloc::string::String;
use alloc::string::ToString;
use builtins::error::Suggestion;

pub fn install_debug_hooks() {
    // Suppress default panic location hook body
    Report::install_debug_hook::<core::panic::Location>(
        |_location: &core::panic::Location, _ctx: &mut HookContext<core::panic::Location>| {
            #[cfg(debug_assertions)]
            _ctx.push_body(format!("at {}", _location));
        },
    );

    // No body for parse errors themselves — only for position context
    Report::install_debug_hook::<crate::error::parse::ParseError>(|_, _| {});

    // Show input line with caret for ParsePosition
    Report::install_debug_hook::<ParsePosition>(
        |ParsePosition { pos, input }, ctx: &mut HookContext<ParsePosition>| {
            let pos = *pos;
            let input = input.as_deref().unwrap_or(&[]);
            if input.is_empty() {
                ctx.push_body(format!("parse error at byte position {}", pos));
            } else {
                let (line, caret_col, caret_len) = format_line_and_caret(input, pos);
                ctx.push_body(line);
                ctx.push_body(crate::caret::caret_line(caret_col, caret_len));
            }
        },
    );

    // Show suggestion for InvalidArgument errors
    Report::install_debug_hook::<Suggestion>(
        |Suggestion(msg), ctx: &mut HookContext<Suggestion>| {
            ctx.push_body(format!("Suggestion: {msg}"));
        },
    );
}

fn format_line_and_caret(input: &[u8], pos: usize) -> (String, usize, usize) {
    let line_start = input
        .get(..pos)
        .and_then(|prefix| prefix.iter().rposition(|&b| b == b'\n').map(|p| p + 1))
        .unwrap_or(0);
    let line_end = input
        .get(pos..)
        .and_then(|suffix| suffix.iter().position(|&b| b == b'\n').map(|p| pos + p))
        .unwrap_or(input.len());
    let line = input
        .get(line_start..line_end)
        .and_then(|s| core::str::from_utf8(s).ok())
        .unwrap_or("?")
        .to_string();
    let caret_col = pos - line_start;
    let caret_len = compute_caret_len(input, pos, line_start);
    (line, caret_col, caret_len)
}

fn compute_caret_len(input: &[u8], pos: usize, line_start: usize) -> usize {
    let mut caret_len = 1;
    let local_pos = pos - line_start;
    let rest = input.get(line_start..).unwrap_or(&[]);
    for &kw in &[
        b"if" as &[u8],
        b"fi",
        b"then",
        b"else",
        b"elif",
        b"for",
        b"while",
        b"until",
        b"done",
    ] {
        let kw_len = kw.len();
        if rest.get(local_pos..local_pos + kw_len) == Some(kw) {
            let after = local_pos + kw_len;
            if after >= rest.len() || rest.get(after).is_some_and(|&b| b.is_ascii_whitespace()) {
                caret_len = kw_len;
                break;
            }
        }
    }
    caret_len
}
