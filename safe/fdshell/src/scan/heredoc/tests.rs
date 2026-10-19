#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
use super::{body_regions, skip, skip_region, unterminated};
use alloc::vec;

/// The delimiter word is a word, so it gets POSIX quote removal in
/// `op::Operator::new`: every `"`/`'` byte is removed (not one surrounding
/// pair — task #207), so the terminator line is the stripped word.
#[test]
fn delimiter_quote_removal_ends_the_body_at_the_stripped_word() {
    // `<<a'b'c` delimits `abc`; the one-pair rule left `'a'b'c'`'s bytes in.
    assert_eq!(skip(b"cat <<a'b'c\nbody\nabc", 0, 11), Some(20));
    assert_eq!(skip(b"cat <<'x'y'z'\nbody\nxyz", 0, 13), Some(22));
    assert_eq!(skip(b"cat <<ab'c'\nbody\nabc", 0, 11), Some(20));
    assert_eq!(skip(b"cat <<'a'bc\nbody\nabc", 0, 11), Some(20));
    // `<<- a'b'c`: the marker form, tab-indented terminator.
    assert_eq!(skip(b"cat <<- a'b'c\n\tbody\n\tabc", 0, 13), Some(24));
    // The `"` form obeys the same rule.
    assert_eq!(skip(b"cat <<a\"b\"c\nbody\nabc", 0, 11), Some(20));
    assert_eq!(skip(b"cat <<\"a b\"\nbody\na b", 0, 11), Some(20));
    assert_eq!(skip(b"cat <<\"\"Q\"x\"\nbody\nQx", 0, 12), Some(20));
    assert_eq!(skip(b"cat <<a\"b c\"d\nbody\nab cd", 0, 13), Some(24));
    // An escaped quote is data, so `<<a\"b` delimits the literal `a"b`.
    assert_eq!(skip(b"cat <<a\\\"b\nbody\na\"b", 0, 10), Some(19));
    // A `<<` written inside a quoted delimiter word is word content, not a
    // second operator: the span is consumed whole at both levels.
    assert_eq!(skip(b"cat <<'a <<b'\nbody\na <<b", 0, 13), Some(24));
    assert_eq!(skip(b"cat <<\"a <<b\"\nbody\na <<b", 0, 13), Some(24));
}

/// The body region of a quoted-delimiter heredoc: the region is the body lines
/// only, and the delimiter word's quotes are gone from the terminator.
#[test]
fn skip_region_of_a_quoted_delimiter_heredoc() {
    let line = b"cat <<a'b'c\nbody\nabc\necho x";
    assert_eq!(skip_region(line, 0, 11), Some(((12, 17), 20, 20)));
    let line = b"cat <<\"a b\"\nbody\na b\necho x";
    assert_eq!(skip_region(line, 0, 11), Some(((12, 17), 20, 20)));
}

/// A quoted delimiter makes the body literal, and a `'…'` span is one word for
/// the byte-level delimiter scan: `<<'a b'` delimits `a b` (byte-level and
/// token-level must agree, LESSONS).
#[test]
fn single_quoted_delimiter_is_literal_and_spans_words() {
    // `cat <<'EOF'` — body literal, terminator `EOF`.
    let out = skip(b"cat <<'EOF'\nbody\nEOF", 0, 11);
    assert_eq!(out, Some(20), "the terminator line ends the body");
    // The `<<'EOF'` operator counts as one at the byte level.
    assert_eq!(
        crate::scan::heredoc::operator_count(b"cat <<'EOF'\nbody\nEOF", 0, 11),
        1
    );
    // A delimiter with a space: the span is the whole quoted word.
    let out = skip(b"cat <<'a b'\nbody\na b", 0, 11);
    assert_eq!(out, Some(20));
    // `<< 'a b'` (bare form): the next word is the quoted span.
    let out = skip(b"cat << 'a b'\nbody\na b", 0, 12);
    assert_eq!(out, Some(21));
    // An unbalanced span is not a quoted word: quote removal strips the lone
    // `'`, so the delimiter is `EOF` and the `'EOF` line no longer matches.
    // At CLI level such a line is a parse error (`unmatched quote`, rc 1), so
    // this is the scan-internal shape only (task #207 re-pin).
    assert_eq!(skip(b"cat <<'EOF\nbody\n'EOF", 0, 10), None);
    // The balanced form terminates at the stripped word.
    assert_eq!(skip(b"cat <<'EOF\nbody\nEOF", 0, 10), Some(19));
}

// The resume index is the newline terminating the last delimiter line, or
// `line.len()` when that line is last.
#[test]
fn skip_resume_positions() {
    assert_eq!(skip(b"cat <<EOF\nbody\nEOF", 0, 9), Some(18));
    assert_eq!(skip(b"cat <<EOF\nbody\nEOF\n", 0, 9), Some(18));
    // Empty body: the delimiter line follows the command line directly.
    assert_eq!(skip(b"cat <<EOF\nEOF", 0, 9), Some(13));
}

// A bare `<<` takes the next word as the delimiter.
#[test]
fn skip_bare_form_uses_next_word() {
    assert_eq!(skip(b"cat << Q\nbody\nQ", 0, 8), Some(15));
}

// The `<<-` form strips leading tabs from the body and the terminator line.
#[test]
fn skip_dash_form_strips_leading_tabs() {
    assert_eq!(skip(b"cat <<-EOF\n\tbody\n\tEOF", 0, 10), Some(21));
    // Tabs-only terminator line: the whole line is stripped away.
    assert_eq!(skip(b"cat <<-EOF\n\tbody\n\t\t\tEOF", 0, 10), Some(23));
}

// `<<- WORD` (marker + separate word) reads the next word as the delimiter.
#[test]
fn skip_dash_form_separate_word() {
    assert_eq!(skip(b"cat <<- EOF\nbody\nEOF", 0, 11), Some(20));
}

// A `<<-` at EOL has no delimiter word: not a heredoc command line.
#[test]
fn skip_dash_form_at_eol_is_none() {
    assert_eq!(skip(b"cat <<-", 0, 7), None);
}

// `<<---` consumes exactly one `-` as the marker; the delimiter is `--`
// (not `---` and not `-`), so the body ends at the `--` line.
#[test]
fn skip_dash_form_consumes_one_dash() {
    assert_eq!(skip(b"cat <<---\nbody\n--", 0, 9), Some(17));
}

// A plain `<<EOF` never strips tabs: a tab-indented terminator does not match.
#[test]
fn skip_plain_form_ignores_tab_indented_terminator() {
    assert_eq!(skip(b"cat <<EOF\n\tEOF", 0, 9), None);
}

// `<<-""` (empty quoted delimiter) is terminated by a tabs-only line, which
// strips to empty just like a blank line terminates `<<""`.
#[test]
fn skip_dash_empty_quoted_delimiter_ends_at_tabs_only_line() {
    let line = b"cat <<-\"\"\nbody\n\t\t\necho A";
    assert_eq!(skip_region(line, 0, 9), Some(((10, 15), 17, 18)));
    assert_eq!(skip(line, 0, 9), Some(17));
}

// The dash form's body region ends at the *line* start (tabs included), so
// the terminator's leading tabs stay part of the unblanked line.
#[test]
fn body_regions_dash_form_ends_at_line_start() {
    let line = b"cat <<-EOF\n\tbody\n\tEOF\necho x";
    assert_eq!(body_regions(line), vec![(11, 17)]);
}

// An attached delimiter may start with a non-word byte (`&x`): the attached
// path takes the raw suffix as-is, where the bare-`<<` fallback rejects a
// next word starting with `<`, `>`, `&`, or `%`.
#[test]
fn skip_attached_delimiter_starting_with_non_word_char() {
    assert_eq!(skip(b"wc -c <<&x\n&x", 0, 10), Some(13));
}

// A near-miss delimiter line is body content, not the terminator.
#[test]
fn skip_near_miss_delimiter_is_body() {
    assert_eq!(skip(b"cat <<EOF\nxEOF\nEOF", 0, 9), Some(18));
}

// The body is opaque to `skip`: quote bytes do not affect the resume.
#[test]
fn skip_body_with_quote_byte() {
    assert_eq!(skip(b"cat <<EOF\n\"a\nEOF", 0, 9), Some(16));
}

// A bare `<<` at EOL has no delimiter word: not a heredoc command line.
#[test]
fn skip_bare_heredoc_at_eol_is_none() {
    assert_eq!(skip(b"cat <<", 0, 6), None);
}

// `;` after the operator ends the run before any newline: no extension.
#[test]
fn skip_semicolon_after_operator_is_none() {
    assert_eq!(skip(b"cat << ;", 0, 7), None);
}

// A missing delimiter line: `None`.
#[test]
fn skip_missing_delimiter_is_none() {
    assert_eq!(skip(b"cat <<NOPE\nbody\nnothing", 0, 10), None);
}

// An attached word before `<<` (no word break) is not an operator.
#[test]
fn skip_attached_word_is_not_operator() {
    assert_eq!(skip(b"x<<EOF\nEOF", 0, 6), None);
}

// `<<<` is the here-string form, not a heredoc operator.
#[test]
fn skip_triple_chevron_is_not_operator() {
    assert_eq!(skip(b"cat <<<EOF\nEOF", 0, 10), None);
}

// A `|` before `<<` (only whitespace between) is a pipeline position: the
// word is a command, not an operator. The run ends at the newline (9).
#[test]
fn skip_pipe_position_is_not_operator() {
    assert_eq!(skip(b"a | <<EOF\nEOF", 0, 9), None);
}

// The region is the body lines only: from the first body byte to the start
// of the last delimiter line; the span ends at the resume newline.
#[test]
fn skip_region_covers_body_lines_only() {
    let line = b"cat <<EOF\nbody\nEOF\necho x";
    assert_eq!(skip_region(line, 0, 9), Some(((10, 15), 18, 18)));
}

// An empty final delimiter is a zero-length line: the statement span extends
// through the blank line's terminating newline.
#[test]
fn skip_region_empty_delimiter_span_includes_blank_line() {
    let line = b"cat <<\"\"\nbody\n\nx";
    assert_eq!(skip_region(line, 0, 8), Some(((9, 14), 14, 15)));
    assert_eq!(skip(b"cat <<\"\"\nbody\n\necho A", 0, 8), Some(14));
}

// `<<` inside double quotes or backticks is not an operator.
#[test]
fn skip_quoted_and_backtick_chevron_is_not_operator() {
    assert_eq!(skip(b"cat \" <<EOF\"\nEOF\"", 0, 12), None);
    assert_eq!(skip(b"cat ` <<EOF`\nEOF`", 0, 12), None);
}

// A single-`<` redirect is not a heredoc operator.
#[test]
fn skip_single_chevron_is_not_operator() {
    assert_eq!(skip(b"cat <file\nile", 0, 9), None);
}

// `<<<` in the run is ignored; the later `<<EOF` is the operator.
#[test]
fn skip_triple_then_double_chevron() {
    assert_eq!(skip(b"cat x <<<y <<EOF\nbody\nEOF", 0, 16), Some(25));
}

// A bare `<<` followed by another operator-shaped word has no delimiter word.
#[test]
fn skip_bare_heredoc_before_chevron_word_is_none() {
    assert_eq!(skip(b"cat << <<x\n<<x", 0, 10), None);
}

// Two identical delimiters: each command-line `<<` consumes its own line.
#[test]
fn skip_two_heredocs_same_delimiter() {
    assert_eq!(skip(b"cat <<Q <<Q\nx\nQ\ny\nQ", 0, 11), Some(19));
}

// Two empty-delimiter operators need two blank lines; one is not enough.
#[test]
fn skip_two_empty_delimiters_need_two_blank_lines() {
    assert_eq!(skip(b"cat <<\"\" <<\"\"\n\n", 0, 13), None);
}

// An empty delimiter with no blank line is unterminated. A zero-byte final
// line (EOF right after the body's newline) never matches.
#[test]
fn skip_empty_delimiter_without_blank_line_is_none() {
    assert_eq!(skip(b"cat <<\"\"\n", 0, 8), None);
    assert_eq!(skip(b"wc -c <<\"\"\nbody\n", 0, 10), None);
}

// A `$( )`-closing `)` does not break the word (the tokenizer keeps
// `$(x)<<EOF` whole), so the attached `<<EOF` is not an operator: the body
// lines are not swallowed. A top-level `)` still breaks the word.
#[test]
fn skip_substitution_closing_paren_is_not_word_break() {
    assert_eq!(skip(b"cat $(x)<<EOF\nb\nEOF", 0, 13), None);
    assert_eq!(skip(b"echo (x)<<EOF\nb\nEOF", 0, 13), Some(19));
}

#[test]
fn body_regions_finds_body_span() {
    let line = b"cat <<EOF\nbody\nEOF\necho x";
    assert_eq!(body_regions(line), vec![(10, 15)]);
}

#[test]
fn body_regions_none_for_plain_statements() {
    assert!(body_regions(b"echo hi").is_empty());
    assert!(body_regions(b"cat <<<hi\necho x").is_empty());
    assert!(body_regions(b"x<<EOF\nEOF").is_empty());
}

// A `<<` inside a comment is comment text, not an operator: it must not
// register a body region or swallow the following delimiter-shaped line.
#[test]
fn body_regions_comment_heredoc_is_not_a_region() {
    assert!(body_regions(b"# <<EOF\na\nEOF").is_empty());
    assert!(body_regions(b"echo hi # <<EOF\nEOF").is_empty());
}

// A heredoc inside a block body is found by the whole-text scan; body quote
// bytes do not disturb the scan.
#[test]
fn body_regions_inside_block_body() {
    let line = b"if t; then cat <<EOF\n\"q\nEOF\nfi";
    assert_eq!(body_regions(line), vec![(21, 24)]);
}

// Two heredoc commands in one text yield one region each, in order.
#[test]
fn body_regions_multiple_commands() {
    let line = b"cat <<A\nx\nA\ncat <<B\ny\nB";
    assert_eq!(body_regions(line), vec![(8, 10), (20, 22)]);
}

// A `;`-terminated command line — a single-line block's arm — claims its body
// from the physical line's end (the segment layer's rule at arm granularity,
// task #311): the region is the body lines, ending at the delimiter line's start.
#[test]
fn body_regions_of_a_semicolon_terminated_arm_line() {
    let line = b"if true; then cat <<T; fi\n{a,b}\nT\n";
    assert_eq!(body_regions(line), vec![(26, 32)]);
    let line = b"for x in a; do cat <<T; done\n{a,b}\nT\n";
    assert_eq!(body_regions(line), vec![(29, 35)]);
    let line = b"case x in x) cat <<T;; esac\n{a,b}\nT\n";
    assert_eq!(body_regions(line), vec![(28, 34)]);
    // The `<<-` form: the region ends at the tab-indented line's start.
    let line = b"if true; then cat <<-T; fi\n\t{a,b}\n\tT\n";
    assert_eq!(body_regions(line), vec![(27, 34)]);
}

// Every `<<` of the line claims one region, in global per-line operator order;
// the runs behind the `;` on the same line are consumed by the resume index,
// so the line's regions are counted once.
#[test]
fn body_regions_of_two_operators_on_one_semicolon_terminated_line() {
    let line = b"cat <<A; cat <<B\nb1\nA\nb2\nB\n";
    assert_eq!(body_regions(line), vec![(17, 20), (22, 25)]);
    let line = b"if true; then cat <<T <<U; fi\nb1\nT\nb2\nU\n";
    assert_eq!(body_regions(line), vec![(30, 33), (35, 38)]);
    // Only the operator's own body blanks: the statements behind the `;` stay
    // tokens, and the run after the region is scanned as usual.
    let line = b"cat <<T; echo hi\nb\nT\necho after";
    assert_eq!(body_regions(line), vec![(17, 19)]);
}

// A `;`-terminated line with no `<<` in the run, a `<<` that is not an operator
// (quotes, `$( … )`, a comment), and a missing delimiter line all keep the
// "no region" outcome: the parse layer's own error, as before.
#[test]
fn body_regions_of_a_semicolon_terminated_run_without_a_region() {
    assert!(body_regions(b"echo hi; echo x\n").is_empty());
    assert!(body_regions(b"cat <<T; echo hi\nb\nNOPE").is_empty());
    assert!(body_regions(b"if true; then echo \"a <<b\"; fi\n").is_empty());
    assert!(
        body_regions(b"if true; then echo $(echo <<b); fi\n").is_empty(),
        "a `<<` inside `$( … )` is substitution text"
    );
    assert!(body_regions(b"if true; then echo hi # <<T; fi\nb\nT\n").is_empty());
}

// `unterminated` is the REPL's "keep reading" signal for a `<<` run.

// No `<<` operator: not a heredoc, not unterminated.
#[test]
fn unterminated_no_operator_is_false() {
    assert!(!unterminated(b"echo hi", 0, 7));
}

// A complete heredoc (delimiter line present): not unterminated.
#[test]
fn unterminated_complete_heredoc_is_false() {
    assert!(!unterminated(b"cat <<EOF\nbody\nEOF", 0, 9));
}

// A missing delimiter line: unterminated.
#[test]
fn unterminated_missing_delimiter_is_true() {
    assert!(unterminated(b"cat <<EOF\nbody", 0, 9));
}

// A bare `<<` at EOL has no delimiter word: not continuable.
#[test]
fn unterminated_bare_chevron_at_eol_is_false() {
    assert!(!unterminated(b"cat <<", 0, 6));
}

// A `;`-terminated run with an operator: the body is read after the whole
// logical line, so a missing delimiter line keeps it a continuation.
#[test]
fn unterminated_semicolon_terminated_is_true() {
    assert!(unterminated(b"cat <<EOF ;", 0, 10));
    assert!(!unterminated(b"cat <<EOF ;\nbody\nEOF", 0, 10));
}

// `<<""` (empty quoted delimiter) awaiting its blank line: unterminated.
#[test]
fn unterminated_empty_quoted_delimiter_is_true() {
    assert!(unterminated(b"cat <<\"\"", 0, 8));
}

/// `semi_terminated_regions`' empty-`ops` guard is **live** (task #261 renamed
/// it from "unreachable"): the run window counts the operator while the line
/// window hides it behind an unclosed `((` region, so the line's `ops` is empty.
/// The guard is what keeps that case from jumping `i` to `body_start` and
/// skipping the line's runs behind the `;`; the valid twin claims its body, as
/// every block-arm pin does.
#[test]
fn semi_terminated_regions_guard_is_live() {
    let line = b"(( x ; cat <<B; fi\nbody\nB";
    assert_eq!(
        super::operator_count(line, 6, 14),
        1,
        "the run window counts its operator"
    );
    assert!(
        super::regions::semi_terminated_regions(line, 6, 14).is_none(),
        "the line window hides it behind the unclosed `((`: the guard fires"
    );
    assert!(
        body_regions(line).is_empty(),
        "and the whole-text scan claims no region"
    );
    let line = b"if true; then cat <<T; fi\nbody\nT";
    assert!(super::regions::semi_terminated_regions(line, 8, 21).is_some());
    assert_eq!(body_regions(line), vec![(26, 31)]);
}
