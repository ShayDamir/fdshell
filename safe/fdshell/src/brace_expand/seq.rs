//! Sequence term `{start..end[.incr]}` parsing (port of bash's
//! `valid_seqterm` and `expand_seqterm`).

pub(super) mod expand;
pub(super) mod mkseq;

pub(super) use expand::expand_seqterm;
pub(super) use mkseq::mkseq;

/// The maximum number of words a single source word may produce.
pub(super) const MAX_WORDS: usize = 65_536;

/// How the elements of a sequence are rendered.
#[derive(Clone, Copy, PartialEq, Eq)]
#[cfg_attr(test, derive(Debug))]
pub(super) enum SeqKind {
    /// Plain decimal (`{1..5}`).
    Int,
    /// Zero-padded to `width` (`{01..5}`).
    ZInt,
    /// One-byte values (`{a..c}`).
    Char,
}

/// A fully parsed, valid sequence term.
#[cfg_attr(test, derive(Debug))]
pub(super) struct SeqSpec {
    pub(super) start: i64,
    pub(super) end: i64,
    pub(super) incr: i64,
    pub(super) kind: SeqKind,
    pub(super) width: usize,
}

/// The term kind a leading byte pair may start; mirrors the minimal check
/// bash does in `valid_seqterm`.
fn kind_of(first: u8, second: Option<u8>, rhs_side: bool) -> Option<SeqKind> {
    if first.is_ascii_digit() || matches!(first, b'+' | b'-') && matches!(second, Some(b'0'..=b'9'))
    {
        return Some(SeqKind::Int);
    }
    if first.is_ascii_alphabetic()
        && if rhs_side {
            matches!(second, None | Some(b'.') | Some(b'}'))
        } else {
            second == Some(b'.')
        }
    {
        return Some(SeqKind::Char);
    }
    None
}

/// Whether `amble` looks like a sequence term: it contains `..`, and both
/// sides are plausibly the same type (bash's `valid_seqterm`). An empty side,
/// or a side starting with `.` (which `kind_of` rejects), is invalid.
pub(super) fn valid_seqterm(amble: &[u8]) -> bool {
    let Some(dot) = amble.windows(2).position(|w| w == b"..".as_slice()) else {
        return false;
    };
    let lhs = amble.get(..dot).unwrap_or_default();
    let rhs = amble.get(dot + 2..).unwrap_or_default();
    // For a one-byte lhs the "second byte" is the `.` of the `..` in the
    // amble (bash looks past the first byte of the term).
    let lt = lhs
        .first()
        .copied()
        .and_then(|l0| kind_of(l0, amble.get(1).copied(), false));
    let rt = rhs
        .first()
        .copied()
        .and_then(|r0| kind_of(r0, rhs.get(1).copied(), true));
    lt == rt && lt.is_some()
}

/// Parse the whole byte slice as a decimal `i64` (bash's `valid_number`).
fn parse_int(s: &[u8]) -> Option<i64> {
    let text = core::str::from_utf8(s).ok()?;
    text.parse::<i64>().ok()
}

/// Parse a leading decimal integer (a `strtoimax` prefix); `None` on no
/// digits or overflow.
fn parse_int_prefix(s: &[u8]) -> Option<(i64, usize)> {
    let sign = usize::from(matches!(s.first(), Some(&b'-') | Some(&b'+')));
    let digits = s.get(sign..)?;
    let n = digits.iter().take_while(|b| b.is_ascii_digit()).count();
    if n == 0 {
        return None;
    }
    let text = core::str::from_utf8(s.get(..sign + n)?).ok()?;
    text.parse::<i64>().ok().map(|v| (v, sign + n))
}
