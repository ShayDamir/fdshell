#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
//! Unit pins for the stage parser (task #292): a stage's first word goes
//! through the statement path's keyword dispatch, so a keyword block written as
//! a stage is parsed as a block (with its clause texts sliced out of the
//! statement), every keyword spelling joins the stage, a `{` at the stage
//! position keeps its pinned parse error, and the heredoc spec shares stay
//! aligned across a block stage — whose share is counted on the **untrimmed**
//! slice, because the block's first `;` sits inside it.

use crate::parse::parse;
use crate::parse::{ParsedLine, Pipeline, Stage};
use crate::redirect::RedirectSource;
use alloc::string::String;
use alloc::vec;
use sys::{Origin, Position, ScriptText};

fn stext(bytes: &[u8]) -> ScriptText {
    ScriptText::new(
        sys::ShortCStr::from_vec(bytes.to_vec()).unwrap(),
        Position::new(1, 1),
        Origin::Shell,
    )
}

/// The pipeline `line` parses to, and the bytes of a stage's text.
fn pipeline(line: &[u8]) -> Pipeline {
    let ParsedLine::Pipeline(pipe) = parse(&stext(line)).unwrap() else {
        panic!("{line:?}: expected a pipeline");
    };
    pipe
}

/// A keyword block behind a `|` is a **block** stage, not a stage whose word 0
/// is the keyword: the `while` stage carries the parsed loop, with its condition
/// and body sliced out of the statement text, and the stage's own text is the
/// block verbatim (what the forked child re-enters).
#[test]
fn a_keyword_stage_is_parsed_as_a_block() {
    let line = b"echo a | while :; do echo b; break; done";
    let pipe = pipeline(line);
    assert_eq!(pipe.stages.len(), 2);
    assert_eq!(pipe.command_at(0).unwrap().command, c"echo".into());
    let Stage::Block(block) = pipe.stages.get(1).unwrap() else {
        panic!("stage 1 is a keyword block");
    };
    assert_eq!(block.text.as_bytes().unwrap(), &line[b"echo a | ".len()..]);
    let ParsedLine::While(wh) = &block.parsed else {
        panic!("the `while` stage parses as a loop");
    };
    assert_eq!(wh.condition.as_bytes().unwrap(), b":");
    assert_eq!(wh.body.as_bytes().unwrap(), b"echo b; break");
}

/// Every keyword spelling opens a stage block, spaced and glued to the pipe.
#[test]
fn every_opener_opens_a_stage_block() {
    for tail in [
        b"if :; then echo y; fi".as_slice(),
        b"for x in 1 2; do echo $x; done",
        b"until ! :; do echo b; break; done",
        b"case x in x) echo y;; esac",
        b"f() { echo F; }",
    ] {
        let line = [b"echo a | ".to_vec(), tail.to_vec()].concat();
        let pipe = pipeline(&line);
        assert!(
            matches!(pipe.stages.get(1), Some(Stage::Block(_))),
            "{line:?}: stage 1 is a block"
        );
        let glued = [b"echo a|".to_vec(), tail.to_vec()].concat();
        let pipe = pipeline(&glued);
        assert!(
            matches!(pipe.stages.get(1), Some(Stage::Block(_))),
            "{glued:?}: the glued pipe opens the block too"
        );
    }
}

/// A block stage in the middle: three stages, the block in the middle, and the
/// `;` / `&&` written after the closer split the statement as usual.
#[test]
fn a_block_stage_in_the_middle_keeps_the_stage_count() {
    let pipe = pipeline(b"echo a | while :; do :; done | cat");
    assert_eq!(pipe.stages.len(), 3);
    assert!(matches!(pipe.stages.get(1), Some(Stage::Block(_))));
    assert_eq!(pipe.command_at(2).unwrap().command, c"cat".into());
    // The closer's `;` ends the statement: the tail is not a stage.
    let ParsedLine::Pipeline(pipe) =
        parse(&stext(b"echo a | while :; do :; done; echo tail")).unwrap()
    else {
        panic!("the closer's `;` ends the pipeline statement");
    };
    assert_eq!(pipe.stages.len(), 2);
}

/// The parse layer is permissive about a `{` stage (its dispatch knows the
/// group parser); the accepted divergence of tasks #186/#190 lives one layer up
/// — the segment scanner keeps a brace at a stage **word data**, so its `;`
/// splits the run and the group is truncated: `tests/group.rs` and
/// `tests/pipeline.rs` pin the measured parse error, and the scanner's rule is
/// `group_at_run`, which the stage arm never consults.
#[test]
fn a_brace_stage_is_a_group_for_the_parser_and_a_word_for_the_scanner() {
    let pipe = pipeline(b"echo a | { echo b; }");
    assert_eq!(pipe.stages.len(), 2);
    let Stage::Block(block) = pipe.stages.get(1).unwrap() else {
        panic!("the parse layer dispatches a brace stage as a group");
    };
    assert!(matches!(&block.parsed, ParsedLine::Group(_)));
}

/// The spec shares stay aligned across a block stage: the block's own `<<`
/// bodies are inside its untrimmed slice, and the stage after it must get its
/// own share (trimming at the block's first `;` would desync the count).
#[test]
fn spec_shares_survive_a_block_stage() {
    let line = b"echo a | while :; do cat <<T; break; done | cat\nbody\nT";
    let pipe = pipeline(line);
    assert_eq!(pipe.stages.len(), 3);
    let Stage::Block(block) = pipe.stages.get(1).unwrap() else {
        panic!("stage 1 is the block stage");
    };
    // The block's body carries the heredoc operator and its body text.
    assert!(
        block
            .text
            .as_bytes()
            .unwrap()
            .starts_with(b"while :; do cat <<T"),
        "{:?}",
        String::from_utf8_lossy(block.text.as_bytes().unwrap())
    );
    assert_eq!(pipe.command_at(2).unwrap().command, c"cat".into());
}

/// A keyword-shaped *argument* of a stage (`echo if`) is still a plain command:
/// the dispatch reads the stage's **first** word only.
#[test]
fn a_keyword_shaped_stage_argument_stays_a_command() {
    let pipe = pipeline(b"echo x | echo if");
    assert_eq!(pipe.stages.len(), 2);
    let Stage::Command(cmd, _) = pipe.stages.get(1).unwrap() else {
        panic!("`echo if` is a command stage");
    };
    assert_eq!(cmd.command, c"echo".into());
    assert_eq!(cmd.args, vec![c"if".into()]);
}

/// A **command** stage carries its own text slice: `echo a | f` gives each stage
/// its own bytes (`echo a`, then `f`), word 0 included, with nothing of the
/// neighbour's words. Task #481 needs the slice at runtime — the forked stage
/// child is where a function name must resolve, and `function_call::call` takes
/// the text the way the plain path hands it to `run_cmd` (the block stage above
/// already carries its slice for that reason) — so this is the parse-side guard
/// of that plumbing: a stage that lost its own slice would report the whole
/// statement and this pin would move.
#[test]
fn a_command_stage_carries_its_own_text_slice() {
    let pipe = pipeline(b"echo a | f");
    assert_eq!(pipe.stages.len(), 2);
    let Stage::Command(cmd, text) = pipe.stages.get(1).unwrap() else {
        panic!("a bare name in stage position is a plain command");
    };
    assert_eq!(
        text.as_bytes().unwrap(),
        b"f",
        "the slice is the stage's own"
    );
    assert_eq!(cmd.command, c"f".into(), "the slice starts at its word 0");
    assert!(cmd.args.is_empty(), "stage 0's words stay in stage 0");
    assert_eq!(pipe.command_at(0).unwrap().args, vec![c"a".into()]);
    assert_eq!(
        pipe.stages.first().unwrap().text().as_bytes().unwrap(),
        b"echo a",
        "stage 0 carries its own slice"
    );
}

/// The spec cursor a block stage **leaves** is where the next stage reads: the
/// block advances it by its own operator count (its `<<` are inside its
/// untrimmed slice), so a heredoc in a stage **behind** a block stage that
/// carries none gets the body of **its own** operator. The stage before the
/// block puts a non-zero cursor in play, which is what separates advancing the
/// cursor by `0` from resetting it: a reset hands the last stage the first
/// body (`bodyA` here, where bash's parse gives it `bodyB`).
#[test]
fn the_cursor_a_block_stage_leaves_feeds_the_stage_behind_it() {
    let line = b"cat <<A | echo a | while :; do :; break; done | cat <<B\nbodyA\nA\nbodyB\nB";
    let pipe = pipeline(line);
    assert_eq!(pipe.stages.len(), 4);
    assert!(matches!(pipe.stages.get(2), Some(Stage::Block(_))));
    assert_eq!(heredoc_body(&pipe, 0), "bodyA\n");
    assert_eq!(heredoc_body(&pipe, 3), "bodyB\n");
}

/// The body of stage `stage`'s single here-doc redirect.
fn heredoc_body(pipe: &Pipeline, stage: usize) -> String {
    let source = &pipe.command_at(stage).unwrap().redirects[0].source;
    let RedirectSource::HereDoc { body, .. } = source else {
        panic!("stage {stage} carries a here-doc redirect");
    };
    String::from_utf8_lossy(body.as_bytes().unwrap_or(&[])).into_owned()
}

/// The stage's first-`;` cut goes through [`span_word`](crate::parse::word_quoted::span_word),
/// so a quoted `";"` is a **word** of the stage: the stage keeps its words and
/// the here-doc operator written behind them (task #336). The ungated text cut
/// truncated the stage at the quoted word, dropped that operator and handed the
/// body to the stage **behind** it — so the two bodies below are the pin.
#[test]
fn a_quoted_semicolon_word_does_not_cut_a_stage() {
    let line = b"cat \";\" << A | cat << B\nbodyA\nA\nbodyB\nB";
    let pipe = pipeline(line);
    assert_eq!(pipe.stages.len(), 2);
    let Stage::Command(cmd, _) = pipe.stages.first().unwrap() else {
        panic!("the quoted separator is an argument word of a command stage");
    };
    assert_eq!(
        cmd.args,
        vec![c";".into()],
        "the word is the separator's text"
    );
    assert_eq!(heredoc_body(&pipe, 0), "bodyA\n");
    assert_eq!(heredoc_body(&pipe, 1), "bodyB\n");
    // The escaped separator is a word too.
    assert_eq!(
        heredoc_body(&pipeline(b"cat \\; << A | cat\nbodyA\nA"), 0),
        "bodyA\n"
    );
}
