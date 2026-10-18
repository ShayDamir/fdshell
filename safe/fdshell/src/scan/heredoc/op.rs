//! One `<<` operator: its delimiter word, the two flags that select how it is
//! read (`<<-` tab stripping, a quoted (literal) delimiter) and the descriptor
//! its body goes to (`N<<word`).

use crate::bytes::fold::fold_quotes;
use alloc::vec::Vec;
use sys::ShortCStr;

/// One `<<` operator: the delimiter word plus the flags and the target
/// descriptor that select how the body is read and where it goes. `Operator`
/// is carried through both levels (the byte scanner and the token parser) so
/// the `<<-` / quoted / escape-folding / io-number decision is made once and
/// cannot diverge between them.
#[derive(Clone)]
pub(crate) struct Operator {
    /// The delimiter word: every quote byte stripped, unquoted escape pairs
    /// folded.
    pub(crate) delim: Vec<u8>,
    /// A quoted delimiter: the body is literal.
    pub(crate) quoted: bool,
    /// The `<<-` form: strip leading tabs from the body and the terminator.
    pub(crate) strip: bool,
    /// The descriptor the body goes to: the io-number of `N<<word`, `0`
    /// (stdin) for the unprefixed form (`scan::heredoc::io::io_prefix`).
    pub(crate) fd: i32,
}

impl Operator {
    /// The operator of a raw delimiter word, in one pass over the raw bytes:
    /// POSIX #4.1 folds every unquoted `\X` pair to `X` (so `<<E\OF` delimits
    /// `EOF`) and POSIX quote removal drops every `"`/`'` marker byte (so
    /// `<<"Q"` delimits `Q` and `<<a'b'c` delimits `abc`). Folding and
    /// stripping share one pass because an escaped quote (`a\'b`) is data and
    /// must not read as a marker (LESSONS). `quoted` reports a quote marker
    /// byte anywhere in the word, `strip` is the `<<-` marker, and `fd` is the
    /// io-number of the `[N]<<` form (`0` for the unprefixed form).
    pub(crate) fn new(raw: &[u8], strip: bool, fd: i32) -> Self {
        let (delim, quoted) = fold_quotes(raw);
        Self {
            delim,
            quoted,
            strip,
            fd,
        }
    }

    /// Whether the delimiter line `words` (no newline) ends this operator's
    /// body: byte-exact against the folded delimiter, or leading-tab-stripped
    /// in the `<<-` form.
    pub(crate) fn matches(&self, words: &[u8]) -> bool {
        let words = if self.strip { untab(words) } else { words };
        words == self.delim.as_slice()
    }

    /// The body bytes of `span`: the `<<-` form drops the leading tabs of
    /// every line, the plain form copies the span verbatim.
    pub(crate) fn body_bytes(&self, span: &[u8]) -> Vec<u8> {
        if self.strip {
            untab_body(span)
        } else {
            span.to_vec()
        }
    }
}

/// The operator word at `line[start..end]` (the `<`s at `start`/`start+1`) as
/// the tab-stripping marker flag plus the attached delimiter word. An empty
/// word means the bare form: the delimiter is the next word (`<< WORD`,
/// `<<- WORD`). Exactly one `-` is consumed, so `<<---` is the marker plus
/// the delimiter `--`.
pub(crate) fn operator_delim(line: &[u8], start: usize, end: usize) -> (bool, &[u8]) {
    match line.get(start + 2..end).unwrap_or_default() {
        [b'-', rest @ ..] => (true, rest),
        raw => (false, raw),
    }
}

/// The attached `<<` form: a delimiter byte follows `<<` (after an optional
/// `<<-` marker). `<<` and `<<-` alone are both bare: their delimiter is the
/// next word, read by the same raw-byte rule the parser uses.
pub(crate) fn attached(line: &[u8], start: usize, end: usize) -> bool {
    !operator_delim(line, start, end).1.is_empty()
}

/// The shared bare-operand rule: a bare `<<` delimiter, a bare
/// `>`/`>>`/`<`/`<>`/`>|` path and a bare `&>`/`&>>` path may not be a
/// separator or another operator. A `)` never forms a token (the tokenizer
/// emits a `;` separator instead), so the `;` arm covers the `<<`-before-`)`
/// case. The operator-byte arm is mask-gated ([`operator_word`]): the gate
/// reads the operand's **quote mask**, never its text alone. Every
/// bare-operand site calls this one function, so the families cannot drift
/// apart.
pub(crate) fn invalid_delimiter(word: &ShortCStr, mask: &[bool]) -> bool {
    word.is_empty() || operator_word(word, mask) || word.eq_bytes(b";") || word.eq_bytes(b"|")
}

/// The operator-byte arm of the bare-operand rule, gated on the operand's own
/// quote mask: `<`, `>`, `&`, `%` make a word an operator only when the byte
/// is **unquoted**. POSIX #4.1 removes quotes after tokenization, so `"&f"`
/// is a plain word where `&f` is an operator. Byte-level twin:
/// `lines::delimiter_span` already gates on the raw span's first byte, so the
/// token level must read the mask and not the word text (LESSONS: byte-level
/// and token-level counts agree). A missing mask bit pads to *unquoted*, the
/// house idiom (`substitute/tilde.rs::prefix_quoted`), so a short/empty mask
/// degrades to the text-only rule. `\X`-escaped operands need no arm: the
/// text starts with `\`, so no arm fires.
pub(crate) fn operator_word(word: &ShortCStr, mask: &[bool]) -> bool {
    !mask.first().copied().unwrap_or(false)
        && (word.starts_with(b"<")
            || word.starts_with(b">")
            || word.starts_with(b"&")
            || word.starts_with(b"%"))
}

/// `span` without its leading tabs — the form a `<<-` delimiter line is
/// compared in. Spaces are never stripped.
fn untab(span: &[u8]) -> &[u8] {
    let n = span.iter().take_while(|&&b| b == b'\t').count();
    span.get(n..).unwrap_or_default()
}

/// `span` with the leading tabs of every line removed — the `<<-` body. A
/// tabs-only line becomes empty, which is what ends a `<<-""` body.
fn untab_body(span: &[u8]) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::new();
    let mut first = true;
    for line in span.split(|&b| b == b'\n') {
        if !first {
            out.push(b'\n');
        }
        first = false;
        out.extend_from_slice(untab(line));
    }
    out
}

#[cfg(test)]
mod tests;
