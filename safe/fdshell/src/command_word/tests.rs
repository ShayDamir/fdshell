#![allow(clippy::unwrap_used)]

use alloc::vec;
use alloc::vec::Vec;
use core::borrow::Borrow;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

use crate::parse::{BuiltinPrefix, CommandLine, Pipeline};
use crate::state::ShellState;

use super::expand;
use super::fields::fields;

fn cell() -> ForkCell<ShellState> {
    ForkCell::new(ShellState::new())
}

fn word(s: &str) -> ShortCStr {
    ShortCStr::from_vec(s.as_bytes().to_vec()).unwrap()
}

fn bytes(cs: &ShortCStr) -> Vec<u8> {
    cs.as_bytes().unwrap().to_vec()
}

fn words(list: &[ShortCStr]) -> Vec<Vec<u8>> {
    list.iter().map(bytes).collect()
}

/// A command line with raw (unquoted) word text: mask all `false`, no quoted
/// span, one arg per word.
fn cmdline(command: &str, args: &[&str]) -> CommandLine {
    CommandLine {
        prefix: BuiltinPrefix::None,
        command: word(command),
        command_mask: vec![false; command.len()],
        command_quoted: false,
        env_assigns: vec![],
        args: args.iter().map(|a| word(a)).collect(),
        args_mask: args.iter().map(|a| vec![false; a.len()]).collect(),
        args_quoted: vec![false; args.len()],
        captures: vec![],
        redirects: vec![],
        pidvar: None,
        bg_force: false,
    }
}

/// A command line whose word 0 was written inside double quotes: every mask
/// bit `true`, plus the quoted span (`"$( … )"` shape).
fn quoted_cmd(command: &str) -> CommandLine {
    let mut cmd = cmdline(command, &[]);
    cmd.command_mask = vec![true; command.len()];
    cmd.command_quoted = true;
    cmd
}

fn fields_of(cmdline: &CommandLine, cell: &ForkCell<ShellState>) -> Vec<Vec<u8>> {
    fields(
        &cmdline.command,
        &cmdline.command_mask,
        cmdline.command_quoted,
        cell,
    )
    .unwrap()
    .iter()
    .map(bytes)
    .collect()
}

/// The fast path: a word with no trigger, no escape pair and no pattern byte is
/// its own single field, so the command line stays borrowed — the common
/// command line allocates nothing for word 0.
#[test]
fn plain_command_word_is_borrowed() {
    let cell = cell();
    let cmd = cmdline("echo", &["hi"]);
    assert!(matches!(
        expand(&cmd, &cell).unwrap().borrow(),
        alloc::borrow::Cow::Borrowed(_)
    ));
}

/// The gate is a raw-text test: `"$(echo echo)"` carries mask `true` on every
/// byte and must still expand (bash's double quotes never suppress command
/// substitution). A mask-gated word would keep it literal.
#[test]
fn quoted_trigger_bytes_still_expand() {
    let cell = cell();
    let cmd = quoted_cmd("$(echo echo)");
    let out = expand(&cmd, &cell).unwrap().into_owned();
    assert_eq!(bytes(&out.command), b"echo");
    assert_eq!(out.command_mask, vec![true; 4]);
}

/// The whole-word form names the command; the backtick form inherits the rule.
#[test]
fn whole_word_substitution_names_the_command() {
    let cell = cell();
    for spelling in ["$(echo echo)", "`echo echo`"] {
        let out = expand(&cmdline(spelling, &["hi"]), &cell)
            .unwrap()
            .into_owned();
        assert_eq!(bytes(&out.command), b"echo", "{spelling}");
        assert_eq!(out.args.len(), 1, "{spelling}");
    }
}

/// A mid-word expansion concatenates with the literal bytes of the word.
#[test]
fn mid_word_expansion_concatenates() {
    let cell = cell();
    let out = expand(&cmdline(r#"$(echo "ec")ho"#, &["hi"]), &cell)
        .unwrap()
        .into_owned();
    assert_eq!(bytes(&out.command), b"echo");
}

/// Multi-field expansion: field 0 is the command, fields 1.. are leading
/// arguments and arrive before the word's own args.
#[test]
fn multi_field_expansion_leads_the_arguments() {
    let cell = cell();
    let out = expand(&cmdline("$(echo echo hi)", &["x"]), &cell)
        .unwrap()
        .into_owned();
    assert_eq!(bytes(&out.command), b"echo");
    assert_eq!(words(&out.args), [b"hi".to_vec(), b"x".to_vec()]);
}

/// An expansion output field the child must not re-expand arrives in the
/// single-quote encoding, and every one of its mask bits is `true`.
#[test]
fn leading_fields_arrive_quote_encoded() {
    let cell = cell();
    let out = expand(&cmdline("$(echo echo '$HOME')", &[]), &cell)
        .unwrap()
        .into_owned();
    assert_eq!(bytes(&out.command), b"echo");
    assert_eq!(bytes(out.args.first().unwrap()), br"\$HOME");
    assert_eq!(out.args_mask.first().unwrap(), &vec![true; 6]);
    assert!(out.args_quoted.first().unwrap());
    let out = expand(&cmdline("$(echo echo '`cmd`')", &[]), &cell)
        .unwrap()
        .into_owned();
    assert_eq!(bytes(out.args.first().unwrap()), br"\`cmd\`");
}

/// bash: an unquoted word 0 whose expansion is empty is not a word — the next
/// field becomes the command. The double form carries no space: a space in
/// the word text is literal content under task #222's provenance rule (and
/// the tokenizer never produces one), so the pure empty-expansion shape is
/// the one bash reaches this path with.
#[test]
fn empty_expansion_consumes_the_next_field() {
    let cell = cell();
    for spelling in ["$(true)", "$(true)$(true)"] {
        let out = expand(&cmdline(spelling, &["echo", "hi"]), &cell)
            .unwrap()
            .into_owned();
        assert_eq!(bytes(&out.command), b"echo", "{spelling}");
        assert_eq!(words(&out.args), vec![b"hi".to_vec()], "{spelling}");
    }
}

/// A *quoted* empty expansion is a word: the (empty) command name, with its
/// arguments intact. The mask cannot say "quoted", only the span can.
#[test]
fn quoted_empty_expansion_is_the_empty_name() {
    let cell = cell();
    let mut cmd = quoted_cmd("$(true)");
    cmd.args = vec![word("echo")];
    cmd.args_mask = vec![vec![false; 4]];
    cmd.args_quoted = vec![false];
    let out = expand(&cmd, &cell).unwrap().into_owned();
    assert_eq!(bytes(&out.command), b"");
    assert_eq!(bytes(out.args.first().unwrap()), b"echo");
}

/// Nothing left to consume: the command is the empty name (fdshell reports a
/// missing command; bash reports `: command not found`).
#[test]
fn empty_expansion_with_no_words_left_is_the_empty_name() {
    let cell = cell();
    let out = expand(&cmdline("$(echo )", &[]), &cell)
        .unwrap()
        .into_owned();
    assert_eq!(bytes(&out.command), b"");
    assert!(out.args.is_empty());
}

/// IFS splitting applies to word 0's expansion, with the shell's own `IFS`.
#[test]
fn ifs_splitting_applies_to_word_zero() {
    let cell = cell();
    cell.borrow_mut().unwrap().ifs = word("x");
    let out = expand(&cmdline(r#"$(echo "echoxhi")"#, &["zz"]), &cell)
        .unwrap()
        .into_owned();
    assert_eq!(bytes(&out.command), b"echo");
    assert_eq!(words(&out.args), [b"hi".to_vec(), b"zz".to_vec()]);
}

/// The escape pair folds in the no-trigger path too (POSIX #4.1): `e\cho` is
/// the command `echo`; a quoted pair survives as the literal name.
#[test]
fn escape_pairs_fold_in_the_no_trigger_path() {
    let cell = cell();
    let out = expand(&cmdline(r"e\cho", &[]), &cell).unwrap().into_owned();
    assert_eq!(bytes(&out.command), b"echo");
    let out = expand(&quoted_cmd(r"e\cho"), &cell).unwrap().into_owned();
    assert_eq!(bytes(&out.command), br"e\cho");
}

/// A leading `~` is a trigger: word 0 gets tilde expansion, and only a leading
/// tilde is one. Tilde reads `$HOME` from the process environment (unset →
/// the tilde stays), so the expectation is built from the same source.
#[test]
fn leading_tilde_takes_the_substitution_path() {
    let cell = cell();
    let home = sys::env::getenv(c"HOME")
        .map(|h| h.as_bytes().unwrap().to_vec())
        .unwrap_or_default();
    let want = [home, b"/bin/ls".to_vec()].concat();
    let out = expand(&cmdline("~/bin/ls", &[]), &cell)
        .unwrap()
        .into_owned();
    assert_eq!(bytes(&out.command), want);
    let out = expand(&cmdline("~bin/ls", &[]), &cell)
        .unwrap()
        .into_owned();
    assert_eq!(bytes(&out.command), b"~bin/ls");
}

/// A parameter expansion in word 0 names the command (`$` is a trigger byte).
#[test]
fn parameter_expansion_names_the_command() {
    let cell = cell();
    cell.borrow_mut().unwrap().set_var(
        ShortCStr::from(c"cmd"),
        sys::ImportedStr::shell(ShortCStr::from(c"echo")),
    );
    let out = expand(&cmdline("$cmd", &["hi"]), &cell)
        .unwrap()
        .into_owned();
    assert_eq!(bytes(&out.command), b"echo");
}

/// Arithmetic in word 0 applies too (`$((1+1))` looks up the command `2`).
#[test]
fn arithmetic_expansion_names_the_command() {
    let cell = cell();
    let out = expand(&cmdline("$((1+1))", &[]), &cell)
        .unwrap()
        .into_owned();
    assert_eq!(bytes(&out.command), b"2");
}

/// `%fd` is an expansion trigger in word 0: a bound name is substituted with a
/// descriptor number (the fd pipeline's own `dup`, so the number is not
/// predictable here) and that number is the command name; an unbound name stays
/// the literal text. Measured with `fdshell -c`: `signalfd %s 10; $(echo %s)`
/// reports the name `"10"`, `%d` reports the name `"%d"`.
#[test]
fn percent_fd_names_the_command() {
    let cell = cell();
    let dev_null = sys::openat2::open(c"/dev/null", 0).unwrap();
    cell.borrow_mut().unwrap().fds.insert(
        ShortCStr::from(c"s"),
        crate::state::FdVar {
            fd: dev_null,
            trace: sys::Trace::boundary(sys::Origin::Shell),
        },
    );
    let out = expand(&cmdline("%s", &[]), &cell).unwrap().into_owned();
    let name = bytes(&out.command);
    assert!(
        !name.is_empty() && name.iter().all(|b| b.is_ascii_digit()),
        "the bound fd becomes the numeric command name, got {name:?}"
    );
    let out = expand(&cmdline("%d", &[]), &cell).unwrap().into_owned();
    assert_eq!(bytes(&out.command), b"%d");
}

/// Every pipeline stage's word 0 is expanded, in stage order.
#[test]
fn pipeline_stages_expand_their_word_zero() {
    let cell = cell();
    let pipeline = Pipeline {
        commands: vec![cmdline("$(echo echo)", &["hi"]), cmdline("cat", &[])],
    };
    let out = super::expand_pipeline(&pipeline, &cell).unwrap();
    let names: Vec<Vec<u8>> = out.commands.iter().map(|c| bytes(&c.command)).collect();
    assert_eq!(names, [b"echo".to_vec(), b"cat".to_vec()]);
}

/// `fields` is the raw field list: the multi-field expansion, and a word with
/// no expansion trigger keeps today's word-0 path (fold, then glob).
#[test]
fn fields_are_the_expanded_field_list() {
    let cell = cell();
    assert_eq!(
        fields_of(&cmdline("$(echo echo hi)", &[]), &cell),
        [b"echo".to_vec(), b"hi".to_vec()]
    );
    assert_eq!(
        fields_of(&cmdline("$(true)", &[]), &cell),
        Vec::<Vec<u8>>::new()
    );
    assert_eq!(fields_of(&cmdline("echo", &[]), &cell), [b"echo".to_vec()]);
    assert_eq!(
        fields_of(&cmdline(r"e\cho", &[]), &cell),
        [b"echo".to_vec()]
    );
}
