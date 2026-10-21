use super::*;
use crate::capture::Capture;
use crate::cd::cd;
use crate::parse::{BuiltinPrefix, CommandLine};
use alloc::format;
use alloc::vec;
use alloc::vec::Vec;
use sys::ImportedStr;
use sys::Origin;
use sys::Position;
use sys::ScriptText;
use sys::ShortCStr;

fn text(bytes: &[u8]) -> ScriptText {
    ScriptText::new(
        ShortCStr::from_vec(bytes.to_vec()).unwrap(),
        Position::new(1, 1),
        Origin::Shell,
    )
}

fn pos() -> Position {
    Position::new(1, 1)
}

fn make_cmdline(args: &[&str]) -> CommandLine {
    let args_vec: Vec<ShortCStr> = args
        .iter()
        .map(|s| ShortCStr::from_vec(s.as_bytes().to_vec()).unwrap())
        .collect();
    CommandLine {
        prefix: BuiltinPrefix::None,
        command: c"cd".into(),
        command_mask: vec![],
        command_span: vec![],
        command_quoted: false,
        env_assigns: vec![],
        args: args_vec,
        args_mask: vec![vec![]; args.len()],
        args_span: vec![vec![]; args.len()],
        args_quoted: vec![false; args.len()],
        captures: vec![],
        redirects: vec![],
        pidvar: None,
        bg_force: false,
    }
}

/// A cmdline carrying a **real** mask per word (`quoted[i]` is every byte's bit
/// of word `i`). An empty mask entry reads as *fully quoted*, so an expansion
/// row must pass the mask (LESSONS, task #279 item 6); the rows above keep the
/// empty masks on purpose — they are the "an empty mask **is** a quoted word"
/// pair.
fn make_cmdline_masked(words: &[&str], quoted: &[bool]) -> CommandLine {
    let mut cmdline = make_cmdline(words);
    cmdline.args_mask = words
        .iter()
        .zip(quoted)
        .map(|(w, &q)| vec![q; w.len()])
        .collect();
    cmdline.args_quoted = quoted.to_vec();
    cmdline
}

fn make_cell() -> ForkCell<ShellState> {
    ForkCell::new(ShellState::new())
}

fn make_line(args: &[&str]) -> Vec<u8> {
    args.join(" ").into_bytes()
}

fn fields(words: &[&str]) -> Vec<ShortCStr> {
    words
        .iter()
        .map(|w| ShortCStr::from_vec(w.as_bytes().to_vec()).unwrap())
        .collect()
}

#[test]
fn cd_to_tmp_success() {
    let line = make_line(&["cd", "/tmp"]);
    let cmdline = make_cmdline(&["/tmp"]);
    let cell = make_cell();
    let result = run_cd(&line, &cmdline, &text(&line), &cell);
    assert!(result.is_ok());
    assert!(result.unwrap());
    let state = cell.borrow().unwrap();
    assert!(state.fds.contains_key::<sys::ShortCStr>(&c"CWD".into()));
}

#[test]
fn cd_dash_switches_to_oldwd() {
    let cell = make_cell();
    {
        let mut state = cell.borrow_mut().unwrap();
        let tmp = c"/tmp".into();
        cd(CdTarget::Path(tmp), &mut state, pos()).unwrap();
        let root = c"/".into();
        cd(CdTarget::Path(root), &mut state, pos()).unwrap();
    }
    let line = make_line(&["cd", "-"]);
    let cmdline = make_cmdline(&["-"]);
    let result = run_cd(&line, &cmdline, &text(&line), &cell);
    assert!(result.is_ok());
    assert!(result.unwrap());
    let state = cell.borrow().unwrap();
    assert!(state.fds.contains_key::<sys::ShortCStr>(&c"OLDCWD".into()));
}

#[test]
fn cd_home_success() {
    let home = std::env::var_os("HOME");
    let Some(home_path) = home else {
        return;
    };
    if !std::path::Path::new(&home_path).exists() {
        return;
    }
    let line = make_line(&["cd"]);
    let cmdline = make_cmdline(&[]);
    let cell = make_cell();
    let result = run_cd(&line, &cmdline, &text(&line), &cell);
    assert!(result.is_ok());
    assert!(result.unwrap());
    let state = cell.borrow().unwrap();
    assert!(state.fds.contains_key::<sys::ShortCStr>(&c"CWD".into()));
}

#[test]
fn cd_missing_var_fails() {
    let line = make_line(&["cd", "%NONEXISTENT"]);
    let cmdline = make_cmdline(&["%NONEXISTENT"]);
    let cell = make_cell();
    let result = run_cd(&line, &cmdline, &text(&line), &cell);
    assert!(result.is_err());
    let report = result.unwrap_err();
    assert!(matches!(report.current_context(), CmdError::Cd));
}

#[test]
fn cd_builtin_not_supported() {
    let line = make_line(&["builtin", "cd", "/tmp"]);
    let cmdline = make_cmdline(&["/tmp"]);
    let mut cmdline = cmdline;
    cmdline.prefix = BuiltinPrefix::Builtin;
    let cell = make_cell();
    let result = run_cd(&line, &cmdline, &text(&line), &cell);
    assert!(result.is_err());
    let report = result.unwrap_err();
    assert!(matches!(
        report.current_context(),
        CmdError::BuiltinKeywordNotSupported { .. }
    ));
}

#[test]
fn cd_captures_not_supported() {
    let line = make_line(&["cd", "/tmp"]);
    let cmdline = make_cmdline(&["/tmp"]);
    let mut cmdline = cmdline;
    cmdline.captures = vec![Capture {
        var: c"fd".into(),
        tag: None,
        force: false,
        cap: None,
        set_at: pos(),
    }];
    let cell = make_cell();
    let result = run_cd(&line, &cmdline, &text(&line), &cell);
    assert!(result.is_err());
    let report = result.unwrap_err();
    assert!(matches!(
        report.current_context(),
        CmdError::CapturesNotSupported { .. }
    ));
}

// ---------------------------------------------------------------------------
// The target decision (`target`): the raw word and the expanded fields.

#[test]
fn target_no_fields_is_home() {
    // The bare `cd` (no word at all) and a 0-field expansion (`cd "$@"` with
    // no positionals, `cd $unset`) reach `target` identically: no word, no
    // fields. The integration pins of `tests/cd_expand.rs` walk both shapes.
    assert!(matches!(target(None, &[]), Ok(CdTarget::Home)));
    assert!(matches!(target(None, &fields(&[])), Ok(CdTarget::Home)));
}

#[test]
fn target_raw_percent_word_is_the_fd_form() {
    // The caller hands the name stripped of its `%`, and the fields are empty
    // because the pipeline never ran on this word.
    let var = ShortCStr::from(c"d");
    let named = target(Some(var.clone()), &[]).unwrap();
    assert!(matches!(&named, CdTarget::Var(n) if n.eq_bytes(b"d")));
    // A word that carries another trigger too (`%d$( … )`) is still the fd
    // form, fields and all: the accepted family owned by task #371.
    let with_fields = target(Some(var), &fields(&["x"])).unwrap();
    assert!(matches!(&with_fields, CdTarget::Var(n) if n.eq_bytes(b"d")));
}

#[test]
fn target_one_field_is_the_path() {
    let one = fields(&["sub"]);
    let target = target(None, &one).unwrap();
    assert!(matches!(&target, CdTarget::Path(p) if p.eq_bytes(b"sub")));
}

#[test]
fn target_dash_field_is_the_old_cwd() {
    // The `-` form is read on the **expanded** field, quoting included
    // (`cd "$D"` with `D=-`), measured against bash 5.3.9.
    let one = fields(&["-"]);
    assert!(matches!(target(None, &one), Ok(CdTarget::OldCwd)));
    // The literal directory named `-` is reached by any other path.
    let literal = fields(&["./-"]);
    let target = target(None, &literal).unwrap();
    assert!(matches!(&target, CdTarget::Path(p) if p.eq_bytes(b"./-")));
}

#[test]
fn target_two_fields_is_the_usage_error() {
    let two = fields(&["a", "b"]);
    let report = target(None, &two).unwrap_err();
    assert!(matches!(report.current_context(), CmdError::CdTooManyArgs));
}

// ---------------------------------------------------------------------------
// `run_cd` over a real mask: the operand is expanded before the target is set.

static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// A pid + counter scratch dir, removed on drop (LESSONS, tasks #255/#342).
struct Scratch(std::path::PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

/// A scratch dir holding `sub/marker`, and a base dir the shell starts in.
fn scratch(tag: &str) -> Scratch {
    let c = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "fdshell-intercept-cd-{}-{}-{}",
        tag,
        std::process::id(),
        c
    ));
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    std::fs::write(dir.join("sub/marker"), b"x").unwrap();
    Scratch(dir)
}

fn base(dir: &std::path::Path) -> ShortCStr {
    ShortCStr::from_vec(dir.to_str().unwrap().as_bytes().to_vec()).unwrap()
}

/// The `cd` of a forked child, so the process cwd the test moves never leaks
/// into its siblings (the shape `cd/tests.rs` uses).
fn child_test(f: impl FnOnce()) {
    let (_, pidfd_opt) = sys::fork_pidfd::fork_pidfd().unwrap();
    match pidfd_opt {
        None => {
            f();
            sys::exit(42);
        }
        Some(pidfd) => {
            let status = pidfd.wait_pidfd().unwrap();
            assert_eq!(status.exit_code(), 42, "the child must exit 42");
        }
    }
}

/// `cd $V` with `$V = sub`: the field the pipeline produced is the directory.
#[test]
fn run_cd_expands_the_operand() {
    let dir = scratch("expand");
    child_test(|| {
        let cell = make_cell();
        {
            let mut state = cell.borrow_mut().unwrap();
            state
                .strings
                .insert(ShortCStr::from(c"V"), ImportedStr::shell(c"sub".into()));
            cd(CdTarget::Path(base(&dir.0)), &mut state, pos()).unwrap();
        }
        let line = make_line(&["cd", "$V"]);
        let cmdline = make_cmdline_masked(&["$V"], &[false]);
        let result = run_cd(&line, &cmdline, &text(&line), &cell);
        assert!(result.is_ok(), "stderr path: {result:?}");
        // The process cwd is the shell's base, so the relative open lands in
        // `sub` only when the expansion drove the move.
        sys::openat2::open(c"marker", sys::fcntl::O_RDONLY).unwrap();
    });
}

/// `cd '$V'`: the tokenizer encodes the `'…'` span as `\$V` with every mask bit
/// set, and the pipeline decodes it back to the literal name — the mask pair of
/// the row above (an all-`false` mask expands, a `'…'` span does not).
#[test]
fn run_cd_single_quoted_operand_stays_literal() {
    let dir = scratch("quoted");
    child_test(|| {
        let cell = make_cell();
        {
            let mut state = cell.borrow_mut().unwrap();
            state
                .strings
                .insert(ShortCStr::from(c"V"), ImportedStr::shell(c"sub".into()));
            cd(CdTarget::Path(base(&dir.0)), &mut state, pos()).unwrap();
        }
        let line = make_line(&["cd", "'$V'"]);
        let cmdline = make_cmdline_masked(&[r"\$V"], &[true; 3]);
        let result = run_cd(&line, &cmdline, &text(&line), &cell);
        let report = result.unwrap_err();
        assert!(matches!(report.current_context(), CmdError::Cd));
    });
}

/// `cd a b` splits into two fields: `cd: too many arguments`, and the shell's
/// base stays where it was.
#[test]
fn run_cd_two_fields_is_the_usage_error() {
    let dir = scratch("too-many");
    child_test(|| {
        let cell = make_cell();
        cd(
            CdTarget::Path(base(&dir.0)),
            &mut cell.borrow_mut().unwrap(),
            pos(),
        )
        .unwrap();
        let line = make_line(&["cd", "a", "b"]);
        let cmdline = make_cmdline_masked(&["a", "b"], &[false, false]);
        let result = run_cd(&line, &cmdline, &text(&line), &cell);
        let report = result.unwrap_err();
        assert!(matches!(report.current_context(), CmdError::CdTooManyArgs));
        sys::openat2::open(c"sub", sys::fcntl::O_RDONLY + sys::fcntl::O_DIRECTORY).unwrap();
    });
}
