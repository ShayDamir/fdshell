//! One `<<` operator: its delimiter word and the two flags that select how
//! it is read — `<<-` tab stripping and a quoted (literal) delimiter.

use super::lines::delimiter_word;
use alloc::vec::Vec;

/// One `<<` operator: the delimiter word plus the two flags that select how
/// the body is read. `Operator` is carried through both levels (the byte
/// scanner and the token parser) so the `<<-` / quoted decision is made once
/// and cannot diverge between them.
#[derive(Clone, Copy)]
pub(crate) struct Operator<'a> {
    /// The delimiter word, quotes stripped.
    pub(crate) delim: &'a [u8],
    /// A quoted delimiter: the body is literal.
    pub(crate) quoted: bool,
    /// The `<<-` form: strip leading tabs from the body and the terminator.
    pub(crate) strip: bool,
}

impl<'a> Operator<'a> {
    /// The operator of a raw delimiter word: one pair of double quotes is
    /// stripped (`<<"Q"` → literal `Q`) and `strip` is the `<<-` marker.
    pub(crate) fn new(raw: &'a [u8], strip: bool) -> Self {
        let (delim, quoted) = delimiter_word(raw);
        Self {
            delim,
            quoted,
            strip,
        }
    }

    /// Whether the delimiter line `words` (no newline) ends this operator's
    /// body: byte-exact, or leading-tab-stripped in the `<<-` form.
    pub(crate) fn matches(&self, words: &[u8]) -> bool {
        let words = if self.strip { untab(words) } else { words };
        words == self.delim
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
