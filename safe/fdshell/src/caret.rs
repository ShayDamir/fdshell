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
