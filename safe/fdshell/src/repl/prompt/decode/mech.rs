// mutants: -C --lib -C --test -C repl -- -E 'test(/^repl::/) | binary(repl)'

//! The escape tables behind the decoder's dispatch: the mechanical escapes
//! of bash's PROMPTING table (`\a \e \n \r \\`), octal at *exactly three*
//! digits, hex as the fdshell extension (decision D1), and the value-escape
//! lookup whose variants live in `segment.rs`.

use crate::repl::prompt::segment::ValueEscape;

/// The byte a mechanical escape maps to (bash's PROMPTING table).
pub(super) fn mechanical(c: u8) -> Option<u8> {
    match c {
        b'a' => Some(0x07),
        b'e' => Some(0x1b),
        b'n' => Some(b'\n'),
        b'r' => Some(b'\r'),
        b'\\' => Some(b'\\'),
        _ => None,
    }
}

/// `\nnn` — **exactly three** octal digits at `at`, measured (the rule that
/// keeps `\12`, `\7` and `\01` literal, unlike `printf`'s 1-to-3 decoder —
/// LESSONS); the value truncates to eight bits (`\777` is `0xff`).
pub(super) fn octal(input: &[u8], at: usize) -> Option<u8> {
    let first = octal_digit(*input.get(at)?)?;
    let second = octal_digit(*input.get(at + 1)?)?;
    let third = octal_digit(*input.get(at + 2)?)?;
    Some((first * 64 + second * 8 + third) as u8)
}

/// `\xHH` — one or two hex digits at `at` (the fdshell extension; bash 5.3.9
/// measures it literal): returns the byte and the digits consumed. No hex
/// digit means no escape.
pub(super) fn hex(input: &[u8], at: usize) -> Option<(u8, usize)> {
    let first = hex_digit(*input.get(at)?)?;
    let Some(d) = input.get(at + 1).copied().and_then(hex_digit) else {
        return Some((first as u8, 1));
    };
    Some(((first * 16 + d) as u8, 2))
}

/// The bash PROMPTING table's value escapes, as the lookup the dispatch
/// calls; the variants (and the siblings that own their values) are in
/// `segment.rs`. `\D{…}` stays unrecognised — that is #618's.
pub(super) fn value_escape(c: u8) -> Option<ValueEscape> {
    match c {
        b'u' => Some(ValueEscape::Username),
        b'h' => Some(ValueEscape::Hostname),
        b'H' => Some(ValueEscape::FullHostname),
        b'l' => Some(ValueEscape::TtyName),
        b'$' => Some(ValueEscape::Dollars),
        b's' => Some(ValueEscape::ShellName),
        b'v' => Some(ValueEscape::Version),
        b'V' => Some(ValueEscape::PlusVersion),
        b'j' => Some(ValueEscape::Jobs),
        b'!' => Some(ValueEscape::History),
        b'#' => Some(ValueEscape::AlignedNumber),
        b'd' => Some(ValueEscape::Date),
        b't' => Some(ValueEscape::Time),
        b'T' => Some(ValueEscape::Time24),
        b'@' => Some(ValueEscape::AmPmTime),
        b'A' => Some(ValueEscape::MinuteTime),
        b'w' => Some(ValueEscape::WorkingDir),
        b'W' => Some(ValueEscape::ShortWorkingDir),
        _ => None,
    }
}

/// One octal digit, the `checked_sub` convention of
/// `child/printf/escapes.rs` (the caller asks about any byte).
fn octal_digit(c: u8) -> Option<u32> {
    let d = c.checked_sub(b'0')?;
    (d <= 7).then_some(d as u32)
}

/// One hex digit, same convention.
fn hex_digit(c: u8) -> Option<u32> {
    if let Some(d) = c.checked_sub(b'0').filter(|d| *d <= 9) {
        return Some(d as u32);
    }
    if let Some(d) = c.checked_sub(b'a').filter(|d| *d <= 5) {
        return Some(u32::from(d) + 10);
    }
    if let Some(d) = c.checked_sub(b'A').filter(|d| *d <= 5) {
        return Some(u32::from(d) + 10);
    }
    None
}

#[cfg(test)]
mod tests;
