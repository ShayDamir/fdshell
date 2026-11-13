#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
use super::*;
use alloc::format;
use alloc::vec;
use alloc::vec::Vec;
use sys::ShortCStr;

/// Every row below is bash 5.3.9's answer, measured row by row (initial mask
/// pinned in the same child): `(initial mask, mode, mask bash leaves)`.
const APPLY: &[(u32, &str, u32)] = &[
    // The copy specs read the initial allowed bits, and they read only the class
    // they name — one bit at a time, so each source class is varied alone.
    (0o022, "u=g", 0o0222),
    (0o020, "u=g", 0o0220),
    (0o060, "u=g", 0o0660),
    (0o010, "u=g", 0o0110),
    (0o400, "g=u", 0o0440),
    (0o200, "g=u", 0o0220),
    (0o100, "g=u", 0o0110),
    (0o044, "u=g", 0o0444),
    (0o400, "o=u", 0o0404),
    (0o200, "o=g", 0o0200),
    (0o100, "u=g", 0o0000),
    (0o400, "o+u", 0o0400),
    (0o700, "a=u", 0o0777),
    (0o700, "u=g", 0o0000),
    (0o700, "u=o", 0o0000),
    (0o711, "u=g", 0o0111),
    (0o711, "o=g", 0o0711),
    (0o171, "u=g", 0o0771),
    (0o171, "g=o", 0o0111),
    (0o141, "u=o", 0o0141),
    (0o121, "u=g", 0o0221),
    (0o111, "a=u", 0o0111),
    // The `who` letters, in either order, and the default `who`.
    (0o022, "au=rw", 0o0111),
    (0o022, "ua=rw", 0o0111),
    (0o022, "ug=u", 0o0002),
    (0o022, "go+u", 0o0000),
    (0o022, "+r", 0o0022),
    (0o022, "+w", 0o0000),
    (0o022, "+x", 0o0022),
    (0o022, "=r", 0o0333),
    (0o022, "+u", 0o0000),
    (0o022, "=u", 0o0000),
    // The three operators, the copy replacing what the run had accumulated.
    (0o022, "u+x", 0o0022),
    (0o022, "u-x", 0o0122),
    (0o022, "u=x", 0o0622),
    (0o022, "o+x", 0o0022),
    (0o022, "g+x", 0o0022),
    (0o022, "u=rwxg", 0o0222),
    (0o022, "u=rwxgo", 0o0222),
    (0o022, "u=rwxs", 0o0022),
    (0o022, "a=rwxt", 0o0000),
    (0o022, "=ugo", 0o0222),
    (0o022, "u=r,g=w,o=x", 0o0356),
    // `X` is the *initial* bits' exec bit, not the running one, and a copy resets
    // the run — so the order of `X` and the copy is visible.
    (0o077, "u=Xg", 0o0777),
    (0o077, "u=gX", 0o0677),
    (0o022, "u=Xg", 0o0222),
    (0o000, "u=gX", 0o0000),
    (0o000, "u=X", 0o0600),
    (0o077, "u=X", 0o0677),
    (0o777, "u=X", 0o0777),
    (0o022, "a=X", 0o0666),
    (0o000, "a-rwx,u=X", 0o0677),
    (0o0777, "g+x,u=X", 0o0767),
    (0o0777, "g+x,u=X,u+x", 0o0667),
    // A letter repeated inside one action run is idempotent — the run accumulates a
    // single permission, so `a+rwr` is `a+rw` and `u=rwr` is `u=rw`. Every row is
    // bash 5.3.9's measured answer.
    (0o777, "a+rwr", 0o0111),
    (0o000, "a-rwr", 0o0666),
    (0o022, "a=rwr", 0o0111),
    (0o000, "u=rwr", 0o0100),
    (0o777, "a+rxr", 0o0222),
    // Several clauses, and an operator restart inside one clause (same `who`).
    (0o022, "u=rwx", 0o0022),
    (0o077, "u=rwx", 0o0077),
    (0o000, "u=rwx", 0o0000),
    (0o000, "g-w", 0o0020),
    (0o022, "a=rwx", 0o0000),
    (0o022, "u=rwx,g=,o=", 0o0077),
    (0o022, "u=g,g=o,o=u", 0o0220),
    (0o022, "u=rwx,g=rx,o=rx,o=u", 0o0020),
    (0o022, "u=r,g=w,o=x,u=g", 0o0256),
    (0o022, "u=r,g=w,o=x,g=o", 0o0326),
    (0o022, "u=r,g=w,o=x,o=g", 0o0352),
    (0o022, "a=r,u=g", 0o0233),
    (0o022, "u=r,g=w,o=x,u=o", 0o0256),
    (0o022, "u+r-w", 0o0222),
    (0o022, "u=rwx,g=r-w+rx", 0o0022),
    (0o022, "u=rw+x", 0o0022),
    (0o022, "u=rwx,u-r,u+x", 0o0422),
    (0o022, "u+g", 0o0022),
    // A bare operator: `+` and `-` add/remove nothing, `=` clears what the `who`
    // covers, and a bare `-` (fdshell's operand, not an option) is the no-op bash
    // measures.
    (0o022, "+", 0o0022),
    (0o022, "=", 0o0777),
    (0o022, "-", 0o0022),
];

/// The `-S` line for each mask: a letter per bit the mask leaves clear. bash 5.3.9,
/// measured; each class is run through all eight of its values.
const SYMBOLIC_TEXT: &[(u32, &str)] = &[
    (0o000, "u=rwx,g=rwx,o=rwx"),
    (0o100, "u=rw,g=rwx,o=rwx"),
    (0o200, "u=rx,g=rwx,o=rwx"),
    (0o300, "u=r,g=rwx,o=rwx"),
    (0o400, "u=wx,g=rwx,o=rwx"),
    (0o500, "u=w,g=rwx,o=rwx"),
    (0o600, "u=x,g=rwx,o=rwx"),
    (0o700, "u=,g=rwx,o=rwx"),
    (0o010, "u=rwx,g=rw,o=rwx"),
    (0o020, "u=rwx,g=rx,o=rwx"),
    (0o030, "u=rwx,g=r,o=rwx"),
    (0o040, "u=rwx,g=wx,o=rwx"),
    (0o050, "u=rwx,g=w,o=rwx"),
    (0o060, "u=rwx,g=x,o=rwx"),
    (0o070, "u=rwx,g=,o=rwx"),
    (0o001, "u=rwx,g=rwx,o=rw"),
    (0o002, "u=rwx,g=rwx,o=rx"),
    (0o003, "u=rwx,g=rwx,o=r"),
    (0o004, "u=rwx,g=rwx,o=wx"),
    (0o005, "u=rwx,g=rwx,o=w"),
    (0o006, "u=rwx,g=rwx,o=x"),
    (0o007, "u=rwx,g=rwx,o="),
    (0o022, "u=rwx,g=rx,o=rx"),
    (0o777, "u=,g=,o="),
    (0o644, "u=x,g=wx,o=wx"),
    (0o111, "u=rw,g=rw,o=rw"),
    (0o240, "u=rx,g=wx,o=rwx"),
];

fn word(s: &str) -> ShortCStr {
    ShortCStr::from_vec(s.as_bytes().to_vec()).unwrap()
}

fn parse(s: &str) -> SymbolicMode {
    SymbolicMode::parse(&word(s)).unwrap()
}

fn mode(clauses: Vec<Clause>) -> SymbolicMode {
    SymbolicMode { clauses }
}

fn clause(who: u32, actions: Vec<Action>) -> Clause {
    Clause { who, actions }
}

fn action(op: Op, perm: Vec<Step>) -> Action {
    Action { op, perm }
}

fn steps(v: &[Step]) -> Vec<Step> {
    v.to_vec()
}

#[test]
fn applying_the_mode_answers_bash_measured_mask() {
    for (initial, word, want) in APPLY {
        assert_eq!(
            parse(word).apply(*initial),
            *want,
            "init {initial:04o} mode {word:?}"
        );
    }
}

#[test]
fn the_report_line_lists_the_bits_the_mask_leaves_clear() {
    for (mask, want) in SYMBOLIC_TEXT {
        assert_eq!(symbolic_text(*mask), *want, "mask {mask:04o}");
    }
}

#[test]
fn the_ast_keeps_who_operator_and_action_run_apart() {
    let rwx = steps(&[Step::Bits(0o444), Step::Bits(0o222), Step::Bits(0o111)]);
    assert_eq!(
        parse("u=rwx"),
        mode(vec![clause(0o700, vec![action(Op::Assign, rwx)])])
    );
    // Every class letter, in any order, and the default `who` of `a`.
    assert_eq!(
        parse("ug=r"),
        mode(vec![clause(
            0o770,
            vec![action(Op::Assign, steps(&[Step::Bits(0o444)]))]
        )])
    );
    // `a` is every class, so the letter before it makes no difference to the AST.
    assert_eq!(
        parse("oa+r"),
        mode(vec![clause(
            0o777,
            vec![action(Op::Add, steps(&[Step::Bits(0o444)]))]
        )])
    );
    assert_eq!(
        parse("=x"),
        mode(vec![clause(
            0o777,
            vec![action(Op::Assign, steps(&[Step::Bits(0o111)]))]
        )])
    );
    // The three operators, on the same action run.
    for op in [Op::Add, Op::Remove, Op::Assign] {
        assert_eq!(
            parse(&format!("u{}x", op_letter(&op))),
            mode(vec![clause(
                0o700,
                vec![action(op, steps(&[Step::Bits(0o111)]))]
            )]),
        );
    }
}

#[test]
fn a_copy_spec_is_a_step_of_its_own_class_and_replaces_the_run() {
    let rwx = steps(&[Step::Bits(0o444), Step::Bits(0o222), Step::Bits(0o111)]);
    assert_eq!(
        parse("u=rwxg"),
        mode(vec![clause(
            0o700,
            vec![action(
                Op::Assign,
                steps(&[
                    Step::Bits(0o444),
                    Step::Bits(0o222),
                    Step::Bits(0o111),
                    Step::Copy(0o070)
                ])
            )]
        )])
    );
    assert_eq!(rwx.len(), 3);
    // The copy letters are the class they name; `X` is its own step, and `s`/`t`
    // are accepted as the empty step (their bits are above `who`).
    // With no `who` letters before the operator, the run's own `u`/`g`/`o` are copy
    // specs, and the default `who` is every class.
    assert_eq!(
        parse("=ugoXs"),
        mode(vec![clause(
            0o777,
            vec![action(
                Op::Assign,
                steps(&[
                    Step::Copy(0o700),
                    Step::Copy(0o070),
                    Step::Copy(0o007),
                    Step::X,
                    Step::Bits(0)
                ])
            )]
        )])
    );
    assert_eq!(
        parse("u=rwxt"),
        mode(vec![clause(
            0o700,
            vec![action(
                Op::Assign,
                steps(&[
                    Step::Bits(0o444),
                    Step::Bits(0o222),
                    Step::Bits(0o111),
                    Step::Bits(0)
                ])
            )]
        )])
    );
}

#[test]
fn a_comma_starts_a_new_clause_and_an_operator_restarts_this_one() {
    // Two clauses: the second one's `who` is parsed again.
    assert_eq!(
        parse("u=r,g=w"),
        mode(vec![
            clause(0o700, vec![action(Op::Assign, steps(&[Step::Bits(0o444)]))]),
            clause(0o070, vec![action(Op::Assign, steps(&[Step::Bits(0o222)]))]),
        ])
    );
    // One clause, two operators, the `who` kept; `perm` restarts at each operator.
    assert_eq!(
        parse("u+r-w"),
        mode(vec![clause(
            0o700,
            vec![
                action(Op::Add, steps(&[Step::Bits(0o444)])),
                action(Op::Remove, steps(&[Step::Bits(0o222)]))
            ]
        )])
    );
}

#[test]
fn the_three_class_bits_are_kept_separately() {
    // One clause per class, each with one letter, so `apply` has to keep the classes
    // apart: only the class's own bit is moved.
    assert_eq!(
        parse("u=r,g=w,o=x"),
        mode(vec![
            clause(0o700, vec![action(Op::Assign, steps(&[Step::Bits(0o444)]))]),
            clause(0o070, vec![action(Op::Assign, steps(&[Step::Bits(0o222)]))]),
            clause(0o007, vec![action(Op::Assign, steps(&[Step::Bits(0o111)]))]),
        ])
    );
    assert_eq!(parse("u=r,g=w,o=x").apply(0o022), 0o356);
}

fn op_letter(op: &Op) -> char {
    match op {
        Op::Add => '+',
        Op::Remove => '-',
        Op::Assign => '=',
    }
}
