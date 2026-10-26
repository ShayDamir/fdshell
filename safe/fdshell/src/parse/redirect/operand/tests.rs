//! The operand-offset table of `operand_offset` (task #324). The offset is the
//! byte index at which a redirect's **operand** starts in the token's own text,
//! and the agreement pin compares it with the operand the parsers derive for
//! the same spelling: two owners of one rule, one pin (LESSONS: a byte-level
//! and a token-level rule must call ONE predicate).

#![allow(clippy::unwrap_used)]
use super::operand_offset;
use crate::parse::Token;
use crate::parse::bg_redirect::parse_bg_redirect;
use crate::parse::redirect::parse_redirect;
use crate::parse::token::tokenize_statement;
use crate::redirect::RedirectSource;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

fn tokens_of(line: &str) -> Vec<Token> {
    tokenize_statement(line.as_bytes()).unwrap()
}

/// Token `i`'s operand region, sliced out of the token's **raw span** exactly
/// as `brace_expand::rebuild` hands it to the guard; `None` = no operand. The
/// raw span is the text with quotes, which is what bash names in
/// `ambiguous redirect` and what `expand_word` needs to see a quoted `{`.
fn region(line: &str, i: usize) -> Option<String> {
    let tokens = tokens_of(line);
    let off = operand_offset(&tokens, i)?;
    let (_t, start, end, _fq, _mask, _spans) = tokens.get(i).unwrap();
    let bytes = line
        .as_bytes()
        .get(*start..*end)
        .unwrap()
        .get(off..)
        .unwrap();
    Some(String::from_utf8(bytes.to_vec()).unwrap())
}

/// The operand text the parser derives for the redirect word at token `op`:
/// the path word, the `%var` name with its `%`, the dup number, the close `-`.
/// A bare operator's operand is the **next** token, so its text is the operand
/// token's whole text.
fn parsed_operand(tokens: &[Token], op: usize) -> Option<String> {
    let source = match parse_redirect(tokens, op).unwrap() {
        Some((def, _n)) => def.source,
        None => {
            let (bg, _n) = parse_bg_redirect(tokens, op).unwrap()?;
            bg.redirects.first().map(|d| d.source.clone())?
        }
    };
    let bytes: Vec<u8> = match source {
        RedirectSource::Path { path, .. } => path.as_bytes().unwrap().to_vec(),
        RedirectSource::Var(name) => [b"%".as_slice(), name.as_bytes().unwrap()].concat(),
        RedirectSource::Dup(n) => format!("{n}").into_bytes(),
        RedirectSource::Close => b"-".to_vec(),
        _ => return None,
    };
    Some(String::from_utf8(bytes).unwrap())
}

/// Every operand spelling and the offset its operand starts at: the attached
/// path arms (with each operator and an io-number prefix), the `%var` arms, the
/// dup arms, the `&>` family, and the bare forms, whose offset is 0 of the
/// **next** token. The last two rows are the quote shapes: the offset is a
/// token-text offset (the tokenizer strips quotes), so a span that carries
/// quote bytes *before* the operand is sliced a little early. A quoted
/// here-doc/here-string word is the parser's operandless arm, and a quoted
/// operand brace needs no shift to be seen as quoted; only the quoted
/// io-number digits shift, and that row is pinned as the documented divergence:
/// the bail itself is bash's, while the message names the operand together with
/// the operator byte and the prefix's closing quote (bash names `{a,b}`).
#[test]
fn operand_offset_is_the_operand_region_of_the_token() {
    for (line, i, want) in [
        ("cat >f", 1, Some("f")),
        ("cat >>f", 1, Some("f")),
        ("cat <>f", 1, Some("f")),
        ("cat >|f", 1, Some("f")),
        ("cat 0<f", 1, Some("f")),
        ("cat 2>f", 1, Some("f")),
        ("cat >x{a,b}", 1, Some("x{a,b}")),
        ("cat >{a,b}x", 1, Some("{a,b}x")),
        ("cat >'{a,b}'", 1, Some("'{a,b}'")),
        ("cat >/dev/fd/3", 1, Some("/dev/fd/3")),
        ("cat >%v", 1, Some("%v")),
        ("cat <&3", 1, Some("3")),
        ("cat >&1", 1, Some("1")),
        ("cat 2>&1", 1, Some("1")),
        ("cat >&-", 1, Some("-")),
        ("cat >&%v", 1, Some("%v")),
        ("cat <&{3,4}", 1, Some("{3,4}")),
        ("cat <&{3,4}x", 1, Some("{3,4}x")),
        ("echo x &>f", 2, Some("f")),
        ("echo x &>>f", 2, Some("f")),
        ("echo x &>{a,b}", 2, Some("{a,b}")),
        ("echo x &>%v", 2, Some("%v")),
        ("cat > f", 2, Some("f")),
        ("cat >> f", 2, Some("f")),
        ("cat <> f", 2, Some("f")),
        ("cat >| f", 2, Some("f")),
        ("cat 2> f", 2, Some("f")),
        ("cat < f", 2, Some("f")),
        ("echo x &> f", 3, Some("f")),
        ("echo x &>> f", 3, Some("f")),
        ("cat > >{a,b}", 2, Some("{a,b}")),
        ("cat >'a'b{c,d}", 1, Some("'a'b{c,d}")),
        ("cat \"2\">{a,b}", 1, Some("\">{a,b}")),
    ] {
        assert_eq!(region(line, i).as_deref(), want, "{line:?} token {i}");
    }
}

/// The agreement pin: the region from `operand_offset` **is** the operand the
/// parser derives for the same word, byte for byte. The `/dev/fd/N` arm is
/// excluded: its path word is mapped to a `Dup(n)` number, so the word is not
/// recoverable from the def (it is pinned in the table above).
#[test]
fn the_operand_slice_is_the_operand_the_parser_derives() {
    for (line, op, operand) in [
        ("cat >f", 1, 1),
        ("cat >>f", 1, 1),
        ("cat <>f", 1, 1),
        ("cat >|f", 1, 1),
        ("cat 2>f", 1, 1),
        ("cat >%v", 1, 1),
        ("cat <&3", 1, 1),
        ("cat 2>&1", 1, 1),
        ("cat >&-", 1, 1),
        ("cat >&%v", 1, 1),
        ("echo x &>f", 2, 2),
        ("echo x &>>f", 2, 2),
        ("echo x &>%v", 2, 2),
        ("cat > f", 1, 2),
        ("cat >> f", 1, 2),
        ("cat <> f", 1, 2),
        ("cat >| f", 1, 2),
        ("cat 2> f", 1, 2),
        ("cat < f", 1, 2),
        ("echo x &> f", 2, 3),
        ("echo x &>> f", 2, 3),
    ] {
        let tokens = tokens_of(line);
        let want = parsed_operand(&tokens, op).unwrap_or_else(|| panic!("{line:?} redirects"));
        assert_eq!(
            region(line, operand).as_deref(),
            Some(want.as_str()),
            "{line:?}"
        );
    }
}

/// The arms with no operand of their own: the pidvar forms, the **bare** `&>` /
/// `&>>` operator token (its operand is the next token, never itself), a word
/// that is not a redirect at all (glued operator, non-digit prefix, quoted
/// operator byte), the here-doc/here-string words, an operator with nothing
/// after it, and a word after a `;` separator — a statement's token list ends
/// at the separator, so a bare operator at a segment end never pairs with the
/// next word.
#[test]
fn an_operandless_word_has_no_operand_offset() {
    for (line, i) in [
        ("echo x &>&p", 2),
        ("echo x &>|&p", 2),
        ("echo x &> f", 2),
        ("echo x &>> f", 2),
        ("echo x \"&>\"{a,b}", 2),
        ("echo x '&>>'{a,b}", 2),
        ("echo x>{a,b}", 1),
        ("exec {a,b}>f", 1),
        ("cat 2'>'f", 1),
        ("cat <<{a,b}", 1),
        ("cat <<< {a,b}", 2),
        ("cat >", 1),
        ("cat >>", 1),
        ("echo x > ; echo {a,b}", 5),
        ("echo {a,b}", 1),
    ] {
        assert_eq!(region(line, i), None, "{line:?} token {i}");
    }
}
