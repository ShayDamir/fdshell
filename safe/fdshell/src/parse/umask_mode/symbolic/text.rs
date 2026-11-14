// mutants: -C --lib -C --test -C umask -- -E 'test(/^parse::umask_mode::/) | binary(umask)'

//! The `-S` line (task #554): how bash prints a mask — one letter per allowed
//! bit. The AST and its evaluation stay in `symbolic`.

use alloc::format;
use alloc::string::String;

/// The body of a `-S` line, bash's `print_symbolic_umask`: a letter per allowed bit.
pub(crate) fn symbolic_text(mask: u32) -> String {
    let (u, g, o) = (class(mask, 0o700), class(mask, 0o070), class(mask, 0o007));
    format!("u={u},g={g},o={o}")
}

/// One class of that line: `r`, `w`, `x` for every bit the mask leaves clear.
fn class(mask: u32, class: u32) -> String {
    let mut text = String::new();
    for (bit, letter) in [(0o444, 'r'), (0o222, 'w'), (0o111, 'x')] {
        if mask & (bit & class) == 0 {
            text.push(letter);
        }
    }
    text
}
