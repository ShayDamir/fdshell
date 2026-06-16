use crate::error::parse::ParseErrorInfo;

/// Format a parser error with fish-like output.
///
/// Shows the offending line and a caret pointing to the error position.
/// Skips the caret if the error is at position 0 on the first line.
pub(crate) fn format_parse_error(input: &[u8], info: &ParseErrorInfo, message: &str) -> String {
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

    // Skip caret at position 0 on first line (obvious)
    let is_first_line = line_start == 0;
    let caret_col = info.source_start - line_start;
    if !(is_first_line && caret_col == 0) {
        output.push_str(std::str::from_utf8(line).unwrap_or("?"));
        output.push('\n');
        output.push_str(&caret_line(caret_col));
        output.push('\n');
    } else {
        output.push_str(std::str::from_utf8(line).unwrap_or("?"));
        output.push('\n');
    }

    output
}

fn caret_line(col: usize) -> String {
    let mut s = String::with_capacity(col + 1);
    for _ in 0..col {
        s.push(' ');
    }
    s.push('^');
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_line_position_zero_no_caret() {
        let input = b"unmatched quote";
        let info = ParseErrorInfo { source_start: 0 };
        let output = format_parse_error(input, &info, "unmatched quote");
        assert_eq!(output, "fdshell: unmatched quote\nunmatched quote\n");
    }

    #[test]
    fn first_line_nonzero_position_shows_caret() {
        let input = b"echo hello world";
        let info = ParseErrorInfo { source_start: 11 };
        let output = format_parse_error(input, &info, "unmatched quote");
        // Position 11 is the 'w' in "world"
        assert_eq!(
            output,
            "fdshell: unmatched quote\necho hello world\n           ^\n"
        );
    }

    #[test]
    fn second_line_shows_caret() {
        // "echo ok\nunmatched quote" — \n is at position 7
        // position 10 is 'm' in "unmatched", line starts at 8, caret_col = 2
        let input = b"echo ok\nunmatched quote";
        let info = ParseErrorInfo { source_start: 10 };
        let output = format_parse_error(input, &info, "unmatched quote");
        assert_eq!(output, "fdshell: unmatched quote\nunmatched quote\n  ^\n");
    }

    #[test]
    fn caret_points_to_correct_column() {
        // "line with error" — position 10 is 'e' in "error"
        let input = b"line with error";
        let info = ParseErrorInfo { source_start: 10 };
        let output = format_parse_error(input, &info, "oops");
        assert_eq!(output, "fdshell: oops\nline with error\n          ^\n");
    }

    #[test]
    fn error_at_line_end() {
        let input = b"cmd;";
        let info = ParseErrorInfo { source_start: 4 };
        let output = format_parse_error(input, &info, "error");
        assert_eq!(output, "fdshell: error\ncmd;\n    ^\n");
    }

    #[test]
    fn caret_skipped_on_first_line_zero() {
        let input = b"error at start";
        let info = ParseErrorInfo { source_start: 0 };
        let output = format_parse_error(input, &info, "parse error");
        assert_eq!(output, "fdshell: parse error\nerror at start\n");
    }
}
