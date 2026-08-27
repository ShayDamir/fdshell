#![allow(clippy::unwrap_used)]
use super::run_source;
use crate::error::cmd::CmdError;
use crate::intercept::try_intercept;
use crate::loop_control::LoopControl;
use crate::parse::CommandLine;
use crate::state::ShellState;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

fn make_cmdline(command: &[u8], args: &[&str]) -> CommandLine {
    let args_vec: Vec<ShortCStr> = args
        .iter()
        .map(|s| ShortCStr::from_vec(s.as_bytes().to_vec()).unwrap())
        .collect();
    CommandLine {
        builtin: false,
        command: ShortCStr::from_vec(command.to_vec()).unwrap(),
        args: args_vec,
        args_mask: vec![vec![]; args.len()],
        captures: vec![],
        redirects: vec![],
        pidvar: None,
        bg_force: false,
    }
}

fn make_cell() -> ForkCell<ShellState> {
    ForkCell::new(ShellState::new())
}

fn text(bytes: &[u8]) -> sys::ScriptText {
    sys::ScriptText::new(
        ShortCStr::from_vec(bytes.to_vec()).unwrap(),
        sys::Position::new(1, 1),
        sys::Origin::Shell,
    )
}

/// A file in the temp dir, removed when the guard drops.
struct TempFile(Option<std::path::PathBuf>);

impl TempFile {
    fn new(name: &str, content: &[u8]) -> (Self, String) {
        let path =
            std::env::temp_dir().join(format!("fdshell-source-{name}-{}", std::process::id()));
        std::fs::write(&path, content).unwrap();
        (Self(Some(path.clone())), path.to_str().unwrap().to_string())
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        if let Some(path) = &self.0 {
            let _ = std::fs::remove_file(path);
        }
    }
}

fn get_string(cell: &ForkCell<ShellState>, name: &ShortCStr) -> Vec<u8> {
    let state = cell.borrow().unwrap();
    state
        .strings
        .get(name)
        .unwrap()
        .value
        .as_bytes()
        .unwrap()
        .to_vec()
}

#[test]
fn run_source_missing_file_argument_fails() {
    let cell = make_cell();
    let e = run_source(
        b"source",
        &make_cmdline(b"source", &[]),
        &text(b"source"),
        &cell,
    )
    .unwrap_err();
    assert!(matches!(e.current_context(), CmdError::SourceNoFile));
}

#[test]
fn run_source_missing_file_fails() {
    let cell = make_cell();
    let cmdline = make_cmdline(b"source", &["/nonexistent-source-test-xxxxxxxx"]);
    let e = run_source(b"source f", &cmdline, &text(b"source f"), &cell).unwrap_err();
    assert!(matches!(e.current_context(), CmdError::SourceOpen));
}

#[test]
fn run_source_directory_fails_on_read() {
    let cell = make_cell();
    let cmdline = make_cmdline(b"source", &["/tmp"]);
    let e = run_source(b"source f", &cmdline, &text(b"source f"), &cell).unwrap_err();
    assert!(matches!(e.current_context(), CmdError::SourceRead));
}

#[test]
fn run_source_nul_byte_content_fails() {
    let (_tmp, path) = TempFile::new("nul", b"A=1\x00B=2\n");
    let cell = make_cell();
    let cmdline = make_cmdline(b"source", &[&path]);
    let e = run_source(b"source f", &cmdline, &text(b"source f"), &cell).unwrap_err();
    assert!(matches!(e.current_context(), CmdError::SourceNul));
}

#[test]
fn run_source_with_captures_fails() {
    let (_tmp, path) = TempFile::new("captures", b"A=1\n");
    let mut cmdline = make_cmdline(b"source", &[&path]);
    cmdline.captures = vec![crate::capture::Capture {
        var: c"fd".into(),
        tag: None,
        force: false,
        cap: None,
        set_at: sys::Position::new(1, 1),
    }];
    let cell = make_cell();
    assert!(run_source(b"source f", &cmdline, &text(b"source f"), &cell).is_err());
}

#[test]
fn run_source_with_redirects_fails() {
    let (_tmp, path) = TempFile::new("redirects", b"A=1\n");
    let mut cmdline = make_cmdline(b"source", &[&path]);
    cmdline.redirects = vec![crate::redirect::RedirectDef {
        export_to: 1,
        direction: crate::redirect::RedirectDirection::Write,
        source: crate::redirect::RedirectSource::Var(c"out".into()),
    }];
    let cell = make_cell();
    assert!(run_source(b"source f", &cmdline, &text(b"source f"), &cell).is_err());
}

#[test]
fn run_source_with_builtin_prefix_fails() {
    let (_tmp, path) = TempFile::new("builtin", b"A=1\n");
    let mut cmdline = make_cmdline(b"source", &[&path]);
    cmdline.builtin = true;
    let cell = make_cell();
    assert!(
        run_source(
            b"builtin source f",
            &cmdline,
            &text(b"builtin source f"),
            &cell
        )
        .is_err()
    );
}

#[test]
fn run_source_executes_in_current_shell() {
    let (_tmp, path) = TempFile::new("assign", b"A=hello\n");
    let cell = make_cell();
    let cmdline = make_cmdline(b"source", &[&path]);
    let control = run_source(b"source f", &cmdline, &text(b"source f"), &cell).unwrap();
    assert!(control.is_none());
    assert_eq!(get_string(&cell, &c"A".into()), b"hello".to_vec());
}

#[test]
fn run_source_extras_become_positional_parameters() {
    let (_tmp, path) = TempFile::new("pos", b"A=$0\nB=$1\n");
    let cell = make_cell();
    let cmdline = make_cmdline(b"source", &[&path, "x", "y"]);
    let control = run_source(b"source f x y", &cmdline, &text(b"source f x y"), &cell).unwrap();
    assert!(control.is_none());
    assert_eq!(get_string(&cell, &c"A".into()), b"x".to_vec());
    assert_eq!(get_string(&cell, &c"B".into()), b"y".to_vec());
}

#[test]
fn run_source_restores_positional_parameters() {
    let (_tmp, path) = TempFile::new("restore", b"A=1\n");
    let cell = make_cell();
    {
        let mut state = cell.borrow_mut().unwrap();
        state
            .positional
            .push_back(sys::ImportedStr::shell(c"old".into()));
    }
    let cmdline = make_cmdline(b"source", &[&path, "x"]);
    let _ = run_source(b"source f x", &cmdline, &text(b"source f x"), &cell).unwrap();
    let state = cell.borrow().unwrap();
    assert_eq!(state.positional.len(), 1);
    assert_eq!(
        state.positional.front().unwrap().value.as_bytes().unwrap(),
        b"old"
    );
}

#[test]
fn run_source_without_extras_keeps_positional_parameters() {
    let (_tmp, path) = TempFile::new("keep", b"A=$0\n");
    let cell = make_cell();
    {
        let mut state = cell.borrow_mut().unwrap();
        state
            .positional
            .push_back(sys::ImportedStr::shell(c"kept".into()));
    }
    let cmdline = make_cmdline(b"source", &[&path]);
    let _ = run_source(b"source f", &cmdline, &text(b"source f"), &cell).unwrap();
    assert_eq!(get_string(&cell, &c"A".into()), b"kept".to_vec());
}

#[test]
fn run_source_break_propagates_loop_control() {
    let (_tmp, path) = TempFile::new("break", b"break\n");
    let cell = make_cell();
    let cmdline = make_cmdline(b"source", &[&path]);
    let control = run_source(b"source f", &cmdline, &text(b"source f"), &cell).unwrap();
    assert!(matches!(control, Some(LoopControl::Break)));
}

#[test]
fn run_source_substitutes_path_argument() {
    let (_tmp, path) = TempFile::new("sub", b"V=1\n");
    let cell = make_cell();
    {
        let file = ShortCStr::from_vec(path.as_bytes().to_vec()).unwrap();
        let mut state = cell.borrow_mut().unwrap();
        state
            .strings
            .insert(c"P".into(), sys::ImportedStr::shell(file));
    }
    let cmdline = make_cmdline(b"source", &[r"$P"]);
    let _ = run_source(b"source $P", &cmdline, &text(b"source $P"), &cell).unwrap();
    assert_eq!(get_string(&cell, &c"V".into()), b"1".to_vec());
}

#[test]
fn run_source_self_recursion_over_limit_fails() {
    let (_tmp, path) = TempFile::new("selfsrc", b"");
    std::fs::write(&path, format!("source {path}\n").as_bytes()).unwrap();
    let cell = make_cell();
    let cmdline = make_cmdline(b"source", &[&path]);
    let e = run_source(b"source f", &cmdline, &text(b"source f"), &cell).unwrap_err();
    assert!(matches!(e.current_context(), CmdError::NestingTooDeep));
}

#[test]
fn try_intercept_source_returns_some() {
    let (_tmp, path) = TempFile::new("dispatch", b"A=1\n");
    let cell = make_cell();
    let cmdline = make_cmdline(b"source", &[&path]);
    assert!(
        try_intercept(&text(b"source f"), &cmdline, &cell)
            .unwrap()
            .is_some()
    );
}

#[test]
fn try_intercept_dot_returns_some() {
    let (_tmp, path) = TempFile::new("dot", b"A=1\n");
    let cell = make_cell();
    let cmdline = make_cmdline(b".", &[&path]);
    assert!(
        try_intercept(&text(b". f"), &cmdline, &cell)
            .unwrap()
            .is_some()
    );
}
