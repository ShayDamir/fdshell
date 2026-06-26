use std::iter::Peekable;

/// Skip from the current position past a `#` comment to the next `\n` (or end of input).
/// Returns the number of bytes consumed (including the `#`).
pub fn skip_comment<B: Iterator<Item = u8>>(bytes: &mut Peekable<B>) -> usize {
    let mut count = 1; // the '#' itself
    while let Some(&next) = bytes.peek() {
        bytes.next();
        count += 1;
        if next == b'\n' {
            break;
        }
    }
    count
}
