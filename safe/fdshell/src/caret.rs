//! Caret line rendering for parse error display.
//!
//! Produces a string of `^` and `~` characters aligned to the error
//! position in the offending input line.

pub(crate) fn caret_line(col: usize, len: usize) -> String {
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
#[allow(clippy::unwrap_used)]
mod tests {
    use super::caret_line;

    #[test]
    fn test_caret_zero_col_one_len() {
        assert_eq!(caret_line(0, 1), "^");
    }

    #[test]
    fn test_caret_zero_col_two_len() {
        assert_eq!(caret_line(0, 2), "^^");
    }

    #[test]
    fn test_caret_zero_col_three_len() {
        assert_eq!(caret_line(0, 3), "^~^");
    }

    #[test]
    fn test_caret_zero_col_four_len() {
        assert_eq!(caret_line(0, 4), "^~~^");
    }

    #[test]
    fn test_caret_zero_col_ten_len() {
        assert_eq!(caret_line(0, 10), "^~~~~~~~~^");
    }

    #[test]
    fn test_caret_col_one_len() {
        assert_eq!(caret_line(1, 1), " ^");
    }

    #[test]
    fn test_caret_col_one_two_len() {
        assert_eq!(caret_line(1, 2), " ^^");
    }

    #[test]
    fn test_caret_col_five_one_len() {
        assert_eq!(caret_line(5, 1), "     ^");
    }

    #[test]
    fn test_caret_col_five_five_len() {
        assert_eq!(caret_line(5, 5), "     ^~~~^");
    }

    #[test]
    fn test_caret_zero_len_any_col() {
        assert_eq!(caret_line(0, 0), "");
        assert_eq!(caret_line(3, 0), "   ");
        assert_eq!(caret_line(10, 0), "          ");
    }

    #[test]
    fn test_caret_large_col() {
        let result = caret_line(100, 1);
        assert_eq!(result.len(), 101);
        assert_eq!(&result[0..100], " ".repeat(100).as_str());
        assert_eq!(&result[100..], "^");
    }

    #[test]
    fn test_caret_large_col_large_len() {
        let result = caret_line(50, 20);
        assert_eq!(result.len(), 70);
        assert_eq!(&result[0..50], " ".repeat(50).as_str());
        assert_eq!(&result[50..51], "^");
        assert_eq!(&result[51..69], "~".repeat(18).as_str());
        assert_eq!(&result[69..70], "^");
    }

    #[test]
    fn test_caret_col_one() {
        assert_eq!(caret_line(1, 1), " ^");
        assert_eq!(caret_line(1, 2), " ^^");
        assert_eq!(caret_line(1, 3), " ^~^");
    }
}
