use super::*;
use crate::capture::Capture;
use crate::cd::cd;
use crate::parse::CommandLine;
use crate::redirect::{RedirectDef, RedirectDirection, RedirectSource};
use alloc::vec;
use alloc::vec::Vec;
use sys::ShortCStr;

fn make_cmdline(args: &[&str]) -> CommandLine {
    let args_vec: Vec<ShortCStr> = args
        .iter()
        .map(|s| ShortCStr::from_vec(s.as_bytes().to_vec()).unwrap())
        .collect();
    CommandLine {
        builtin: false,
        command: ShortCStr::from_vec(b"cd".to_vec()).unwrap(),
        args: args_vec,
        args_fq: vec![false; args.len()],
        captures: vec![],
        redirects: vec![],
        pidvar: None,
        bg_force: false,
    }
}

fn make_cell() -> ForkCell<ShellState> {
    ForkCell::new(ShellState::new())
}

fn make_line(args: &[&str]) -> Vec<u8> {
    args.join(" ").into_bytes()
}

#[test]
fn cd_to_tmp_success() {
    let line = make_line(&["cd", "/tmp"]);
    let cmdline = make_cmdline(&["/tmp"]);
    let cell = make_cell();
    let result = run_cd(&line, &cmdline, &cell);
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
        cd(&[tmp], &mut state).unwrap();
        let root = c"/".into();
        cd(&[root], &mut state).unwrap();
    }
    let line = make_line(&["cd", "-"]);
    let cmdline = make_cmdline(&["-"]);
    let result = run_cd(&line, &cmdline, &cell);
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
    let result = run_cd(&line, &cmdline, &cell);
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
    let result = run_cd(&line, &cmdline, &cell);
    assert!(result.is_err());
    let report = result.unwrap_err();
    assert!(matches!(report.current_context(), CmdError::Cd));
}

#[test]
fn cd_builtin_not_supported() {
    let line = make_line(&["builtin", "cd", "/tmp"]);
    let cmdline = make_cmdline(&["/tmp"]);
    let mut cmdline = cmdline;
    cmdline.builtin = true;
    let cell = make_cell();
    let result = run_cd(&line, &cmdline, &cell);
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
        var: ShortCStr::from_vec(b"fd".to_vec()).unwrap(),
        tag: None,
        force: false,
    }];
    let cell = make_cell();
    let result = run_cd(&line, &cmdline, &cell);
    assert!(result.is_err());
    let report = result.unwrap_err();
    assert!(matches!(
        report.current_context(),
        CmdError::CapturesNotSupported { .. }
    ));
}

#[test]
fn cd_redirects_not_supported() {
    let line = make_line(&["cd", "/tmp"]);
    let cmdline = make_cmdline(&["/tmp"]);
    let mut cmdline = cmdline;
    cmdline.redirects = vec![RedirectDef {
        export_to: 1,
        direction: RedirectDirection::Write,
        source: RedirectSource::Var(ShortCStr::from_vec(b"test".to_vec()).unwrap()),
    }];
    let cell = make_cell();
    let result = run_cd(&line, &cmdline, &cell);
    assert!(result.is_err());
    let report = result.unwrap_err();
    assert!(matches!(
        report.current_context(),
        CmdError::RedirectNotSupported { .. }
    ));
}
