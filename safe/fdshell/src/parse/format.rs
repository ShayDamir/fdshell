use crate::error::parse::ParseErrorInfo;

/// Format a parser error with fish-like output.
///
/// Shows the offending line and a caret pointing to the error position.
/// If a shell keyword starts at the error position, the caret covers the full keyword.
/// Uses `info.message` if present, otherwise falls back to a generic message.
pub(crate) fn format_parse_error(input: &[u8], info: &ParseErrorInfo) -> String {
    let message = info.message.unwrap_or("parse error");
    let mut output = format!("fdshell: {message}\n");

    // Find line boundaries
    let line_start = input
        .get(..info.source_start)
        .and_then(|prefix| prefix.iter().rposition(|&b| b == b'\n').map(|p| p + 1))
        .unwrap_or(0);
    let line_end = input
        .get(info.source_start..)
        .and_then(|suffix| {
            suffix
                .iter()
                .position(|&b| b == b'\n')
                .map(|p| info.source_start + p)
        })
        .unwrap_or(input.len());

    let line = input.get(line_start..line_end).unwrap_or(&[]);
    let caret_col = info.source_start - line_start;
    let caret_len = keyword_caret_len(input, info.source_start);

    output.push_str(std::str::from_utf8(line).unwrap_or("?"));
    output.push('\n');
    output.push_str(&caret_line(caret_col, caret_len));
    output.push('\n');

    output
}

/// Return the number of carets to show at the error position.
/// If a shell keyword starts there, cover the full keyword; otherwise 1.
fn keyword_caret_len(input: &[u8], pos: usize) -> usize {
    let line_start = input
        .get(..pos)
        .and_then(|prefix| prefix.iter().rposition(|&b| b == b'\n').map(|p| p + 1))
        .unwrap_or(0);
    let rest = input.get(line_start..).unwrap_or(&[]);
    let local_pos = pos - line_start;

    for &kw in KEYWORDS {
        let kw_len = kw.len();
        if rest.get(local_pos..local_pos + kw_len) == Some(kw) {
            let after = local_pos + kw_len;
            if after >= rest.len() || rest.get(after).is_some_and(|&b| b.is_ascii_whitespace()) {
                return kw_len;
            }
        }
    }
    1
}

const KEYWORDS: &[&[u8]] = &[
    b"if", b"fi", b"then", b"else", b"elif", b"for", b"while", b"until", b"done",
];

fn caret_line(col: usize, len: usize) -> String {
    let mut s = String::with_capacity(col + len);
    for _ in 0..col {
        s.push(' ');
    }
    match len {
        0 => {}
        1 => s.push('^'),
        _ => {
            s.push('^');
            for _ in 2..len {
                s.push('~');
            }
            s.push('^');
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_line_position_zero_shows_caret() {
        let input = b"unmatched quote";
        let info = ParseErrorInfo {
            source_start: 0,
            message: None,
        };
        let output = format_parse_error(input, &info);
        assert_eq!(output, "fdshell: parse error\nunmatched quote\n^\n");
    }

    #[test]
    fn keyword_at_error_position_shows_full_caret() {
        let input = b"if test fi";
        let info = ParseErrorInfo::new(0, "missing 'then'");
        let output = format_parse_error(input, &info);
        assert_eq!(output, "fdshell: missing 'then'\nif test fi\n^^\n");
    }

    #[test]
    fn long_keyword_caret_tilde_pattern() {
        let input = b"while test fi";
        let info = ParseErrorInfo::new(0, "missing end");
        let output = format_parse_error(input, &info);
        assert_eq!(output, "fdshell: missing end\nwhile test fi\n^~~~^\n");
    }

    #[test]
    fn keyword_caret_on_second_line() {
        let input = b"echo\nif test fi";
        let info = ParseErrorInfo::new(5, "missing 'then'");
        let output = format_parse_error(input, &info);
        assert_eq!(output, "fdshell: missing 'then'\nif test fi\n^^\n");
    }

    #[test]
    fn first_line_nonzero_position_shows_caret() {
        let input = b"echo hello world";
        let info = ParseErrorInfo {
            source_start: 11,
            message: None,
        };
        let output = format_parse_error(input, &info);
        assert_eq!(
            output,
            "fdshell: parse error\necho hello world\n           ^\n"
        );
    }

    #[test]
    fn second_line_shows_caret() {
        let input = b"echo ok\nunmatched quote";
        let info = ParseErrorInfo {
            source_start: 10,
            message: None,
        };
        let output = format_parse_error(input, &info);
        assert_eq!(output, "fdshell: parse error\nunmatched quote\n  ^\n");
    }

    #[test]
    fn caret_points_to_correct_column() {
        let input = b"line with error";
        let info = ParseErrorInfo {
            source_start: 10,
            message: None,
        };
        let output = format_parse_error(input, &info);
        assert_eq!(
            output,
            "fdshell: parse error\nline with error\n          ^\n"
        );
    }

    #[test]
    fn error_at_line_end() {
        let input = b"cmd;";
        let info = ParseErrorInfo {
            source_start: 4,
            message: None,
        };
        let output = format_parse_error(input, &info);
        assert_eq!(output, "fdshell: parse error\ncmd;\n    ^\n");
    }

    #[test]
    fn custom_message_with_caret() {
        let input = b"if true; then echo hi";
        let info = ParseErrorInfo::new(0, "missing 'fi'");
        let output = format_parse_error(input, &info);
        assert_eq!(output, "fdshell: missing 'fi'\nif true; then echo hi\n^^\n");
    }
}
