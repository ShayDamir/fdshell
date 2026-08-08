use super::*;
use crate::parse::CommandLine;
use alloc::format;
use alloc::vec;
use alloc::vec::Vec;
use sys::ShortCStr;

fn make_cmdline(command: &[u8], args: &[&str]) -> CommandLine {
    let args_vec: Vec<ShortCStr> = args
        .iter()
        .map(|s| ShortCStr::from_vec(s.as_bytes().to_vec()).unwrap())
        .collect();
    CommandLine {
        builtin: false,
        command: ShortCStr::from_vec(command.to_vec()).unwrap(),
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

fn make_line(command: &str, args: &[&str]) -> Vec<u8> {
    if args.is_empty() {
        command.into()
    } else {
        format!("{} {}", command, args.join(" ")).into_bytes()
    }
}

#[test]
fn try_intercept_cd_returns_true() {
    let line = make_line("cd", &["/tmp"]);
    let cmdline = make_cmdline(b"cd", &["/tmp"]);
    let cell = make_cell();
    assert!(try_intercept(&line, &cmdline, &cell).unwrap());
}

#[test]
fn try_intercept_envfilter_returns_true() {
    let line = make_line("envfilter", &["--list"]);
    let cmdline = make_cmdline(b"envfilter", &["--list"]);
    let cell = make_cell();
    assert!(try_intercept(&line, &cmdline, &cell).unwrap());
}

#[test]
fn try_intercept_shift_returns_true() {
    let line = make_line("shift", &[]);
    let cmdline = make_cmdline(b"shift", &[]);
    let cell = make_cell();
    assert!(try_intercept(&line, &cmdline, &cell).unwrap());
}

#[test]
fn try_intercept_read_returns_true() {
    let line = make_line("read", &["var1"]);
    let cmdline = make_cmdline(b"read", &["-u", "0", "var1"]);
    let cell = make_cell();
    assert!(try_intercept(&line, &cmdline, &cell).unwrap());
}

#[test]
fn try_intercept_unknown_returns_false() {
    let line = make_line("unknown_xyzzy", &[]);
    let cmdline = make_cmdline(b"unknown_xyzzy", &[]);
    let cell = make_cell();
    assert!(!try_intercept(&line, &cmdline, &cell).unwrap());
}
