use super::*;
use crate::capture::Capture;
use crate::parse::CommandLine;
use crate::redirect::{RedirectDef, RedirectDirection, RedirectSource};
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

#[test]
fn try_intercept_export_fd_with_captures_returns_error() {
    let line = make_line("export_fd", &["%tag", "%var"]);
    let mut cmdline = make_cmdline(b"export_fd", &["%tag", "%var"]);
    cmdline.captures = vec![Capture {
        var: c"fd".into(),
        tag: None,
        force: false,
    }];
    let cell = make_cell();
    let result = try_intercept(&line, &cmdline, &cell);
    assert!(
        result.is_err(),
        "export_fd with captures should return an error"
    );
}

#[test]
fn try_intercept_export_fd_with_redirects_returns_error() {
    let line = make_line("export_fd", &["%tag", "%var"]);
    let mut cmdline = make_cmdline(b"export_fd", &["%tag", "%var"]);
    cmdline.redirects = vec![RedirectDef {
        export_to: 1,
        direction: RedirectDirection::Write,
        source: RedirectSource::Var(c"test".into()),
    }];
    let cell = make_cell();
    let result = try_intercept(&line, &cmdline, &cell);
    assert!(
        result.is_err(),
        "export_fd with redirects should return an error"
    );
}

#[test]
fn builtin_pos_found_at_correct_position() {
    let line = make_line("builtin envfilter", &["--allow", "PATH"]);
    let mut cmdline = make_cmdline(b"envfilter", &["--allow", "PATH"]);
    cmdline.builtin = true;
    let cell = make_cell();
    let result = try_intercept(&line, &cmdline, &cell);
    assert!(result.is_err());
    let e = result.unwrap_err();
    let pos: usize = e
        .downcast_ref::<crate::error::parse::ParsePosition>()
        .unwrap()
        .pos;
    assert_eq!(
        pos, 0,
        "builtin should be detected at position 0 (start of line)"
    );

    // Test with builtin in the middle — position should be 4
    let line2 = make_line("cmd builtin envfilter", &["--allow", "PATH"]);
    let mut cmdline2 = make_cmdline(b"envfilter", &["--allow", "PATH"]);
    cmdline2.builtin = true;
    let result2 = try_intercept(&line2, &cmdline2, &cell);
    assert!(result2.is_err());
    let e2 = result2.unwrap_err();
    let pos2: usize = e2
        .downcast_ref::<crate::error::parse::ParsePosition>()
        .unwrap()
        .pos;
    assert_eq!(
        pos2, 4,
        "builtin should be detected at position 4 (after 'cmd ')"
    );
}

#[test]
fn capture_pos_found_at_correct_position() {
    let mut cmdline = make_cmdline(b"envfilter", &["--allow", "PATH"]);
    cmdline.captures = vec![Capture {
        var: c"fd".into(),
        tag: None,
        force: false,
    }];
    let cell = make_cell();
    let result = try_intercept(&b"envfilter --allow %>fd PATH"[..], &cmdline, &cell);
    assert!(result.is_err());
    let e = result.unwrap_err();
    let pos: usize = e
        .downcast_ref::<crate::error::parse::ParsePosition>()
        .unwrap()
        .pos;
    assert_eq!(pos, 18, "%> should be detected at position 18");
}

#[test]
fn redirect_pos_found_at_correct_position() {
    let mut cmdline = make_cmdline(b"envfilter", &["--allow", "PATH"]);
    cmdline.redirects = vec![RedirectDef {
        export_to: 1,
        direction: RedirectDirection::Write,
        source: RedirectSource::Var(c"test".into()),
    }];
    let cell = make_cell();

    let result = try_intercept(&b"envfilter --allow PATH < input"[..], &cmdline, &cell);
    assert!(result.is_err());
    let e = result.unwrap_err();
    let pos: usize = e
        .downcast_ref::<crate::error::parse::ParsePosition>()
        .unwrap()
        .pos;
    assert_eq!(pos, 23, "< should be detected at position 23");

    let result2 = try_intercept(&b"cmd > output"[..], &cmdline, &cell);
    assert!(result2.is_err());
    let e2 = result2.unwrap_err();
    let pos2: usize = e2
        .downcast_ref::<crate::error::parse::ParsePosition>()
        .unwrap()
        .pos;
    assert_eq!(pos2, 4, "> should be detected at position 4");
}
