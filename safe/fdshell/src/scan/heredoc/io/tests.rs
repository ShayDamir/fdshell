//! Tests for the io-number shape rule shared by the byte scan and the token
//! parser: `io_prefix` (raw bytes, walking back from the `<<`), `span_operator`
//! (the `<<` operator's raw span), `string_span` (the `<<<` operator's raw
//! span) and the fd rule delegated to `parse::redirect::parse_fd`.

#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
use super::{digit_run, io_fd, io_prefix, span_operator, string_span};
use crate::scan::ScanState;

/// The `<<` of a probe line: the byte index of its first `<`.
fn chevron(line: &[u8]) -> usize {
    line.iter().position(|&b| b == b'<').unwrap()
}

/// The io-number prefix, at the byte level: the digit run that begins the
/// operator's word, with the run's start index. The rule is syntactic, so the
/// run that overflows `i32` is a prefix too (`io_fd` reports it).
#[test]
fn io_prefix_reads_a_word_initial_digit_run() {
    for (line, op, run, digits) in [
        (b"cat 3<<T".as_slice(), 5usize, 4usize, "3"),
        (b"cat 0<<T", 5, 4, "0"),
        (b"cat 003<<-T", 7, 4, "003"),
        (b"cat 12 34<<T", 9, 7, "34"),
        (b"cat ;3<<T", 6, 5, "3"),
        (b"3<<T", 1, 0, "3"),
        (
            b"cat 99999999999999999999999<<T",
            27,
            4,
            "99999999999999999999999",
        ),
    ] {
        let (start, got) = io_prefix(line, op, &ScanState::new()).unwrap();
        assert_eq!(start, run, "line={:?}", line);
        assert_eq!(got, digits.as_bytes(), "line={:?}", line);
    }
}

/// `None` for every prefix that is not a word-initial digit run: the
/// unprefixed form (`<<T`), a non-digit prefix (`x<<T`, `-3<<T`, `$FD<<T`), a
/// digit run glued to a longer word (`x3<<T`, `>3<<T`), a quoted prefix
/// (`"3"<<T`), a quoted operator (`3"<<"T`), an escaped digit (`\3<<T` — the
/// pair shields the digit, so the run is not word-initial), and the pipeline
/// position (`a|3<<T`, where `word_start` excludes `|`).
#[test]
fn io_prefix_is_none_outside_the_numbered_form() {
    for line in [
        b"cat <<T".as_slice(),
        b"cat <<-T",
        b"cat x<<T",
        b"cat -3<<T",
        b"cat $FD<<T",
        b"cat x3<<T",
        b"cat >3<<T",
        b"cat \"3\"<<T",
        b"cat 3\"<<\"T",
        b"cat \\3<<T",
        b"cat 3\\\\<<T",
        b"cat a|3<<T",
        b"cat a>b3<<T",
        b"cat\tx<<3<<T",
    ] {
        assert!(
            io_prefix(line, chevron(line), &ScanState::new()).is_none(),
            "line={:?}",
            line
        );
    }
}

/// A `<<<` is the here-string: never a here-doc operator, prefixed or not.
#[test]
fn io_prefix_never_reads_a_here_string() {
    for line in [b"cat 3<<<T".as_slice(), b"cat <<<T"] {
        assert!(io_prefix(line, chevron(line), &ScanState::new()).is_none());
    }
}

/// The token-level half: the operator's raw span is `[io-number] << [not `<`]`,
/// and the prefix length is what the delimiter read is shifted by (`3<<T` is
/// attached on `T`, `3<< T` takes the next word).
#[test]
fn span_operator_reports_the_chevron_and_prefix() {
    for (line, start, end, op, prefix) in [
        (b"cat 3<<T".as_slice(), 4usize, 8usize, 5usize, 1usize),
        (b"cat <<T", 4, 7, 4, 0),
        (b"cat 3<< T", 4, 7, 5, 1),
        (b"cat 3<<-T", 4, 9, 5, 1),
        (b"cat ;3<<T", 5, 9, 6, 1),
        (b"cat 003<<U", 4, 10, 7, 3),
    ] {
        let (got, pfx) = span_operator(line, start, end).unwrap();
        assert_eq!((got, pfx), (op, prefix), "line={:?}", line);
    }
}

/// Quote removal and escape folding make the token *text* of all these rows
/// read like a here-doc operator (`3<<T`), while their raw spans are word data.
/// The span rule is what keeps them 0 at the token level, where a token-text
/// rule would count 1 and the byte scan 0 (LESSONS, tasks #261/#274).
#[test]
fn span_operator_is_none_for_word_data() {
    for (line, start, end) in [
        (b"cat x<<T".as_slice(), 4usize, 8usize),
        (b"cat x3<<T", 4, 9),
        (b"cat 3\"<<\"T", 4, 10),
        (b"cat \"3\"<<T", 4, 10),
        (b"cat 3'<<'T", 4, 10),
        (b"cat \\3<<T", 4, 9),
        (b"cat 3\\\\<<T", 4, 10),
        (b"cat 3<<<x", 4, 9),
        (b">3<<T", 0, 5),
        (b"cat $FD<<T", 4, 10),
    ] {
        assert!(span_operator(line, start, end).is_none(), "line={:?}", line);
    }
}

/// The fd rule is the `N>file` rule itself (`parse::redirect::parse_fd`), shared
/// by `N<file`, `N<<word` and `N<<<word`: leading zeros fold, and a run that
/// does not fit `i32` is `None` — the byte scan counts the operator either way
/// and the parse site reports it.
#[test]
fn io_fd_is_the_redirect_fd_rule() {
    for (digits, want) in [
        (b"3".as_slice(), Some(3)),
        (b"0", Some(0)),
        (b"003", Some(3)),
        (b"10", Some(10)),
        (b"2147483647", Some(i32::MAX)),
        (b"2147483648", None),
        (b"99999999999999999999999", None),
        (b"", Some(0)),
    ] {
        assert_eq!(io_fd(digits), want, "digits={:?}", digits);
    }
}

/// The digit walk both count levels share: the run's first digit, `None` when
/// the operator is not preceded by a digit. The rule is syntactic — a run
/// glued to a longer word (`x3<<T`) **is** a run; the callers reject it by
/// comparing the run's start with the span/token-word start.
#[test]
fn digit_run_walks_back_from_the_operator() {
    for (line, op, want) in [
        (b"cat 3<<<T".as_slice(), 5usize, Some(4usize)),
        (b"cat 3<<T", 5, Some(4)),
        (b"cat 003<<<U", 7, Some(4)),
        (b"3<<T", 1, Some(0)),
        (b"cat <<<T", 4, None),
        (b"cat <<T", 4, None),
        (b"cat x<<T", 5, None),
        (b"cat x3<<T", 6, Some(5)),
        (b"cat 3\"<<\"T", 6, None),
        (b"cat \\3<<T", 6, Some(5)),
    ] {
        assert_eq!(digit_run(line, op), want, "line={:?}", line);
    }
}

/// The `<<<` operator's raw span, mirroring `span_operator`: the prefix length
/// is what the word and its mask are sliced by, so `3<<<x` is attached on `x`
/// and `3<<< x` takes the next word (the unshifted test would read `<<x`).
#[test]
fn string_span_reports_the_chevron_and_prefix() {
    for (line, start, end, op, prefix) in [
        (b"cat 3<<<T".as_slice(), 4usize, 9usize, 5usize, 1usize),
        (b"cat <<<T", 4, 8, 4, 0),
        (b"cat 3<<< T", 4, 8, 5, 1),
        (b"cat <<< T", 4, 7, 4, 0),
        (b"cat ;3<<<T", 5, 10, 6, 1),
        (b"cat 003<<<U", 4, 11, 7, 3),
        (b"cat 12<<<x", 4, 10, 6, 2),
        (b"3<<<<<x", 0, 7, 1, 1),
    ] {
        let (got, pfx) = string_span(line, start, end).unwrap();
        assert_eq!((got, pfx), (op, prefix), "line={:?}", line);
    }
}

/// The here-string's span rule is as raw-byte as the here-doc's: quote removal
/// and escape folding make the token *text* of all these rows read `3<<<x`,
/// while their raw spans are word data. A token-text rule would count an
/// operator here and none at the byte scan (LESSONS, tasks #261/#274).
#[test]
fn string_span_is_none_for_word_data() {
    for (line, start, end) in [
        (b"cat \"3\"<<<x".as_slice(), 4usize, 12usize),
        (b"cat \\3<<<x", 4, 10),
        (b"cat 3\"<<<\"x", 4, 11),
        (b"cat 3'<<<'x", 4, 11),
        (b"cat x3<<<y", 4, 10),
        (b"cat -3<<<T", 4, 10),
        (b"cat $FD<<<T", 4, 11),
        (b"cat 3\\\\<<<T", 4, 11),
        (b"cat a|3<<<T", 4, 11),
        (b">3<<<x", 0, 6),
        (b"cat x\" 3<<<T\"", 4, 13),
        (b"cat <<T", 4, 7),
        (b"cat <<-T", 4, 8),
        (b"cat 3<<T", 4, 8),
    ] {
        assert!(string_span(line, start, end).is_none(), "line={:?}", line);
    }
}
