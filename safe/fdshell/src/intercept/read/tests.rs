use super::read_from_fd::read_line_from_fd;
use super::*;
use crate::capture::Capture;
use crate::parse::CommandLine;
use crate::redirect::{RedirectDef, RedirectDirection, RedirectSource};
use alloc::format;
use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;
use sys::ShortCStr;
use sys::SyscallError;
use sys::siginfo::WaitStatus;

fn make_read_cmdline(args: &[ShortCStr]) -> CommandLine {
    CommandLine {
        builtin: false,
        command: c"read".into(),
        args_fq: vec![false; args.len()],
        args: args.to_vec(),
        captures: vec![],
        redirects: vec![],
        pidvar: None,
        bg_force: false,
    }
}

fn make_read_cell() -> ForkCell<ShellState> {
    ForkCell::new(ShellState::new())
}

fn make_read_line(args: &[&str]) -> Vec<u8> {
    args.join(" ").into_bytes()
}

fn text(bytes: &[u8]) -> sys::ScriptText {
    sys::ScriptText::new(
        ShortCStr::from_vec(bytes.to_vec()).unwrap(),
        sys::Position::new(1, 1),
        sys::Origin::Shell,
    )
}

#[test]
fn test_split_fields_single() {
    let fields = split_fields(b"hello world", 1);
    assert_eq!(fields, vec![b"hello world".to_vec()]);
}

#[test]
fn test_split_fields_two_exact() {
    let fields = split_fields(b"hello world", 2);
    assert_eq!(fields, vec![b"hello".to_vec(), b"world".to_vec()]);
}

#[test]
fn test_split_fields_two_extra() {
    let fields = split_fields(b"a b c d", 2);
    assert_eq!(fields, vec![b"a".to_vec(), b"b c d".to_vec()]);
}

#[test]
fn test_split_fields_two_few() {
    let fields = split_fields(b"hello", 3);
    assert_eq!(fields, vec![b"hello".to_vec(), Vec::new(), Vec::new()]);
}

#[test]
fn test_split_fields_tabs() {
    let fields = split_fields(b"a\tb\tc", 3);
    assert_eq!(fields, vec![b"a".to_vec(), b"b".to_vec(), b"c".to_vec()]);
}

#[test]
fn test_split_fields_leading_spaces() {
    let fields = split_fields(b"  a  b  ", 3);
    assert_eq!(fields, vec![b"a".to_vec(), b"b".to_vec(), Vec::new()]);
}

#[test]
fn test_no_targets_error() {
    let args: Vec<ShortCStr> = vec![];
    let result = collect_targets(&args);
    assert!(result.is_err());
}

#[test]
fn test_fdvar_target_rejected() {
    let args = vec![c"%myvar".into()];
    let result = collect_targets(&args);
    assert!(result.is_err());
}

// parse_flags tests

#[test]
fn test_parse_flags_empty() {
    let args: Vec<ShortCStr> = vec![];
    let (source, max_bytes, prompt) = parse_flags(&args).unwrap();
    assert!(matches!(source, SourceFd::Stdin));
    assert!(max_bytes.is_none());
    assert!(prompt.is_none());
}

#[test]
fn test_parse_flags_u_numeric() {
    let args = vec![c"-u".into(), c"3".into()];
    let (source, _, _) = parse_flags(&args).unwrap();
    assert!(matches!(source, SourceFd::RawFd(_)));
}

#[test]
fn test_parse_flags_u_negative() {
    let args = vec![c"-u".into(), c"-1".into()];
    let (source, _, _) = parse_flags(&args).unwrap();
    assert!(matches!(source, SourceFd::RawFd(_)));
}

#[test]
fn test_parse_flags_u_fdvar() {
    let args = vec![c"-u".into(), c"%MYVAR".into()];
    let (source, _, _) = parse_flags(&args).unwrap();
    assert!(matches!(source, SourceFd::FdVar(v) if v.as_bytes().unwrap() == b"MYVAR"));
}

#[test]
fn test_parse_flags_u_invalid() {
    let args = vec![c"-u".into(), c"abc".into()];
    let (source, _, _) = parse_flags(&args).unwrap();
    assert!(matches!(source, SourceFd::RawFd(_)));
}

#[test]
fn test_parse_flags_n_positive() {
    let args = vec![c"-n".into(), c"10".into()];
    let (_, max_bytes, _) = parse_flags(&args).unwrap();
    assert_eq!(max_bytes, Some(10));
}

#[test]
fn test_parse_flags_n_zero() {
    let args = vec![c"-n".into(), c"0".into()];
    let (_, max_bytes, _) = parse_flags(&args).unwrap();
    assert_eq!(max_bytes, Some(0));
}

#[test]
fn test_parse_flags_n_invalid() {
    let args = vec![c"-n".into(), c"abc".into()];
    let result = parse_flags(&args);
    assert!(result.is_err());
}

#[test]
fn test_parse_flags_p_prompt() {
    let args = vec![c"-p".into(), c"Enter: ".into()];
    let (_, _, prompt) = parse_flags(&args).unwrap();
    assert_eq!(prompt, Some(b"Enter: " as &[u8]));
}

#[test]
fn test_parse_flags_combined() {
    let args = vec![
        c"-u".into(),
        c"3".into(),
        c"-n".into(),
        c"5".into(),
        c"-p".into(),
        c"hi".into(),
    ];
    let (source, max_bytes, prompt) = parse_flags(&args).unwrap();
    assert!(matches!(source, SourceFd::RawFd(_)));
    assert_eq!(max_bytes, Some(5));
    assert_eq!(prompt, Some(b"hi" as &[u8]));
}

#[test]
fn test_parse_flags_u_missing_arg() {
    let args = vec![c"-u".into()];
    let result = parse_flags(&args);
    assert!(result.is_err());
}

#[test]
fn test_parse_flags_n_missing_arg() {
    let args = vec![c"-n".into()];
    let result = parse_flags(&args);
    assert!(result.is_err());
}

#[test]
fn test_parse_flags_p_missing_arg() {
    let args = vec![c"-p".into()];
    let result = parse_flags(&args);
    assert!(result.is_err());
}

#[test]
fn test_parse_flags_unknown_arg_ignored() {
    let args = vec![c"-x".into(), c"value".into()];
    let (source, max_bytes, prompt) = parse_flags(&args).unwrap();
    assert!(matches!(source, SourceFd::Stdin));
    assert!(max_bytes.is_none());
    assert!(prompt.is_none());
}

#[test]
fn test_parse_flags_multiple_u_last_wins() {
    let args = vec![c"-u".into(), c"3".into(), c"-u".into(), c"5".into()];
    let (source, _, _) = parse_flags(&args).unwrap();
    assert!(matches!(source, SourceFd::RawFd(_)));
}

// collect_targets tests

#[test]
fn test_collect_targets_single() {
    let args = vec![c"var1".into()];
    let targets = collect_targets(&args).unwrap();
    assert_eq!(targets, vec![c"var1".into()]);
}

#[test]
fn test_collect_targets_multiple() {
    let args = vec![c"a".into(), c"b".into(), c"c".into()];
    let targets = collect_targets(&args).unwrap();
    assert_eq!(targets.len(), 3);
}

#[test]
fn test_collect_targets_skips_flags() {
    let args = vec![
        c"-u".into(),
        c"3".into(),
        c"-n".into(),
        c"5".into(),
        c"var1".into(),
    ];
    let targets = collect_targets(&args).unwrap();
    assert_eq!(targets, vec![c"var1".into()]);
}

#[test]
fn test_collect_targets_fdvar_in_targets_rejected() {
    let args = vec![c"var1".into(), c"%fd".into()];
    let result = collect_targets(&args);
    assert!(result.is_err());
}

// read_from_fd tests

#[test]
fn test_read_line_from_fd_eof() {
    let (read_end, write_end) = sys::pipe::pipe2(0).unwrap();
    // Close write end immediately → EOF
    drop(write_end);

    let mut buf = Vec::new();
    let mut eof = false;
    read_line_from_fd(|b: &mut [u8]| read_end.read(b), &mut buf, &mut eof, None).unwrap();
    assert!(eof);
    assert!(buf.is_empty());
}

#[test]
fn test_read_line_from_fd_max_bytes() {
    let (read_end, write_end) = sys::pipe::pipe2(0).unwrap();
    let data = b"hello world";
    sys::rw::write(&write_end, data).unwrap();
    drop(write_end);

    let mut buf = Vec::new();
    let mut eof = false;
    read_line_from_fd(|b: &mut [u8]| read_end.read(b), &mut buf, &mut eof, Some(5)).unwrap();
    assert_eq!(buf, b"hello");
}

#[test]
fn test_read_line_from_fd_stops_at_newline() {
    let (read_end, write_end) = sys::pipe::pipe2(0).unwrap();
    let data = b"line1\nline2";
    sys::rw::write(&write_end, data).unwrap();
    drop(write_end);

    let mut buf = Vec::new();
    let mut eof = false;
    read_line_from_fd(|b: &mut [u8]| read_end.read(b), &mut buf, &mut eof, None).unwrap();
    assert_eq!(buf, b"line1");
}

#[test]
fn test_read_line_from_fd_error() {
    let mut buf = Vec::new();
    let mut eof = false;
    let result = read_line_from_fd(
        |_b: &mut [u8]| Err(SyscallError::EBADF("read")),
        &mut buf,
        &mut eof,
        None,
    );
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err().current_context(),
        CmdError::Read
    ));
    assert!(!eof);
    assert!(buf.is_empty());
}

#[test]
fn test_read_line_from_fd_multi_chunk() {
    let (read_end, write_end) = sys::pipe::pipe2(0).unwrap();
    let data = [b'x'; 8192];
    sys::rw::write(&write_end, &data).unwrap();
    sys::rw::write(&write_end, b"\n").unwrap();
    drop(write_end);

    let mut buf = Vec::new();
    let mut eof = false;
    read_line_from_fd(|b: &mut [u8]| read_end.read(b), &mut buf, &mut eof, None).unwrap();
    assert!(!eof);
    assert_eq!(buf.len(), 8192);
    assert!(buf.into_iter().all(|b| b == b'x'));
}

// read_line tests via SourceFd::RawFd

#[test]
fn test_read_line_rawfd_eof() {
    let (read_end, write_end) = sys::pipe::pipe2(0).unwrap();
    drop(write_end);

    let exported = read_end.export().unwrap();
    let source = SourceFd::RawFd(
        sys::ShortCStr::from_vec(format!("{}", exported.as_raw()).into_bytes()).unwrap(),
    );
    let result = read_line(&source, None, None);
    assert!(result.is_ok());
    let (buf, eof) = result.unwrap();
    assert!(eof);
    assert!(buf.is_empty());
}

#[test]
fn test_read_line_rawfd_data() {
    let (read_end, write_end) = sys::pipe::pipe2(0).unwrap();
    let data = b"hello world\n";
    sys::rw::write(&write_end, data).unwrap();
    drop(write_end);

    let exported = read_end.export().unwrap();
    let source = SourceFd::RawFd(
        sys::ShortCStr::from_vec(format!("{}", exported.as_raw()).into_bytes()).unwrap(),
    );
    let result = read_line(&source, None, None);
    assert!(result.is_ok());
    let (buf, eof) = result.unwrap();
    assert!(!eof);
    assert_eq!(buf, b"hello world");
}

#[test]
fn test_read_line_rawfd_max_bytes() {
    let (read_end, write_end) = sys::pipe::pipe2(0).unwrap();
    let data = b"hello world\n";
    sys::rw::write(&write_end, data).unwrap();
    drop(write_end);

    let exported = read_end.export().unwrap();
    let source = SourceFd::RawFd(
        sys::ShortCStr::from_vec(format!("{}", exported.as_raw()).into_bytes()).unwrap(),
    );
    let result = read_line(&source, None, Some(5));
    assert!(result.is_ok());
    let (buf, eof) = result.unwrap();
    assert!(!eof);
    assert_eq!(buf, b"hello");
}

#[test]
fn test_read_line_rawfd_stops_at_newline() {
    let (read_end, write_end) = sys::pipe::pipe2(0).unwrap();
    let data = b"first\nsecond\n";
    sys::rw::write(&write_end, data).unwrap();
    drop(write_end);

    let exported = read_end.export().unwrap();
    let source = SourceFd::RawFd(
        sys::ShortCStr::from_vec(format!("{}", exported.as_raw()).into_bytes()).unwrap(),
    );
    let result = read_line(&source, None, None);
    assert!(result.is_ok());
    let (buf, eof) = result.unwrap();
    assert!(!eof);
    assert_eq!(buf, b"first");
}

#[test]
fn test_read_line_fdvar_no_clone() {
    let source = SourceFd::FdVar(c"MYVAR".into());
    let result = read_line(&source, None, None);
    assert!(result.is_ok());
    let (buf, eof) = result.unwrap();
    assert!(!eof);
    assert!(buf.is_empty());
}

#[test]
fn test_read_line_fdvar_with_clone() {
    let (read_end, write_end) = sys::pipe::pipe2(0).unwrap();
    let data = b"from var\n";
    sys::rw::write(&write_end, data).unwrap();
    drop(write_end);

    let source = SourceFd::FdVar(c"MYVAR".into());
    let result = read_line(&source, Some(&read_end), None);
    assert!(result.is_ok());
    let (buf, eof) = result.unwrap();
    assert!(!eof);
    assert_eq!(buf, b"from var");
}

// read_line tests via SourceFd::Stdin (fork + dup2 onto fd 0)

fn stdin_read_line(data: &[u8], max_bytes: Option<usize>) -> (Vec<u8>, bool) {
    let (res_r, res_w) = sys::pipe::pipe2(0).unwrap();
    let (data_r, data_w) = sys::pipe::pipe2(0).unwrap();
    if !data.is_empty() {
        sys::rw::write(&data_w, data).unwrap();
    }
    drop(data_w);

    match sys::fork_pidfd::fork_pidfd().unwrap().1 {
        None => {
            data_r.export_to(0).unwrap();
            drop(data_r);
            let (buf, eof) = match read_line(&SourceFd::Stdin, None, max_bytes) {
                Ok(v) => v,
                Err(_) => sys::exit(3),
            };
            sys::rw::write(&res_w, &buf).unwrap();
            let flag: [u8; 1] = [if eof { 1 } else { 0 }];
            sys::rw::write(&res_w, &flag).unwrap();
            drop(res_w);
            sys::exit(0);
        }
        Some(pidfd) => {
            drop(res_w);
            let mut out = Vec::new();
            let mut chunk = [0u8; 4096];
            loop {
                let n = res_r.read(&mut chunk).unwrap();
                if n == 0 {
                    break;
                }
                if let Some(part) = chunk.get(..n) {
                    out.extend_from_slice(part);
                }
            }
            match sys::wait_pidfd::wait_pidfd(&pidfd).unwrap() {
                WaitStatus::Exited(0) => {}
                other => panic!("child failed: {}", other.exit_code()),
            }
            let eof = out.pop().unwrap_or(0) == 1;
            (out, eof)
        }
    }
}

#[test]
fn test_read_line_stdin_data() {
    let (buf, eof) = stdin_read_line(b"abc\n", None);
    assert!(!eof);
    assert_eq!(buf, b"abc");
}

#[test]
fn test_read_line_stdin_eof() {
    let (buf, eof) = stdin_read_line(b"", None);
    assert!(!eof);
    assert!(buf.is_empty());
}

#[test]
fn test_read_line_stdin_max_bytes() {
    let (buf, eof) = stdin_read_line(b"hello world\n", Some(5));
    assert!(!eof);
    assert_eq!(buf, b"hello");
}

// words.rs edge cases

#[test]
fn test_split_fields_empty_data() {
    let fields = split_fields(b"", 1);
    assert_eq!(fields, vec![b"".to_vec()]);
}

#[test]
fn test_split_fields_empty_data_multiple() {
    let fields = split_fields(b"", 3);
    assert_eq!(fields, vec![b"".to_vec(), Vec::new(), Vec::new()]);
}

#[test]
fn test_split_fields_only_spaces() {
    let fields = split_fields(b"   ", 2);
    assert_eq!(fields, vec![Vec::new(), Vec::new()]);
}

#[test]
fn test_split_fields_trailing_space() {
    let fields = split_fields(b"hello ", 2);
    assert_eq!(fields, vec![b"hello".to_vec(), Vec::new()]);
}

#[test]
fn test_split_fields_mixed_separators() {
    let fields = split_fields(b"a  b\tc", 3);
    assert_eq!(fields, vec![b"a".to_vec(), b"b".to_vec(), b"c".to_vec()]);
}

// collect.rs edge cases

#[test]
fn test_collect_targets_with_flags_and_vars() {
    let args = vec![
        c"-u".into(),
        c"3".into(),
        c"-n".into(),
        c"10".into(),
        c"-p".into(),
        c"prompt".into(),
        c"var1".into(),
        c"var2".into(),
    ];
    let targets = collect_targets(&args).unwrap();
    assert_eq!(targets.len(), 2);
}

#[test]
fn test_collect_targets_dollar_var_allowed() {
    let args = vec![c"$FOO".into()];
    let targets = collect_targets(&args).unwrap();
    assert_eq!(targets, vec![c"$FOO".into()]);
}

// run_read integration tests

fn make_read_u_cmdline(args: &[ShortCStr], fd: i32) -> CommandLine {
    let fd_str = ShortCStr::from_vec(fd.to_string().into_bytes()).unwrap();
    let mut all: Vec<ShortCStr> = vec![c"-u".into(), fd_str];
    all.extend(args.iter().cloned());
    make_read_cmdline(&all)
}

fn make_read_u_line(args: &[ShortCStr], fd: i32) -> Vec<u8> {
    let fd_str = fd.to_string();
    let mut result = b"read -u ".to_vec();
    result.extend(fd_str.into_bytes());
    for (i, arg) in args.iter().enumerate() {
        if i > 0 {
            result.push(b' ');
        }
        result.extend(arg.as_bytes().unwrap());
    }
    result
}

#[test]
fn run_read_simple() {
    let (read_end, write_end) = sys::pipe::pipe2(0).unwrap();
    let data = b"hello world\n";
    sys::rw::write(&write_end, data).unwrap();
    drop(write_end);

    let exported = read_end.export().unwrap();
    let fd = exported.as_raw();
    let line = make_read_u_line(&[c"var1".into(), c"var2".into()], fd);
    let cmdline = make_read_u_cmdline(&[c"var1".into(), c"var2".into()], fd);
    let cell = make_read_cell();

    let result = run_read(&line, &cmdline, &text(&line), &cell);
    assert!(result.is_ok());
    assert!(result.unwrap());

    let state = cell.borrow().unwrap();
    assert_eq!(
        state
            .strings
            .get::<ShortCStr>(&c"var1".into())
            .map(|v| &v.value),
        Some(&c"hello".into())
    );
    assert_eq!(
        state
            .strings
            .get::<ShortCStr>(&c"var2".into())
            .map(|v| &v.value),
        Some(&c"world".into())
    );
}

#[test]
fn run_read_eof_returns_status_1() {
    let (read_end, write_end) = sys::pipe::pipe2(0).unwrap();
    drop(write_end);

    let exported = read_end.export().unwrap();
    let fd = exported.as_raw();
    let line = make_read_u_line(&[c"var1".into()], fd);
    let cmdline = make_read_u_cmdline(&[c"var1".into()], fd);
    let cell = make_read_cell();

    let result = run_read(&line, &cmdline, &text(&line), &cell);
    assert!(result.is_ok());
    assert!(result.unwrap());

    let state = cell.borrow().unwrap();
    assert!(matches!(state.last_status, WaitStatus::Exited(1)));
}

#[test]
fn run_read_builtin_not_supported() {
    let line = make_read_line(&["builtin", "read", "var1"]);
    let cmdline = make_read_cmdline(&[c"var1".into()]);
    let mut cmdline = cmdline;
    cmdline.builtin = true;
    let cell = make_read_cell();
    let result = run_read(&line, &cmdline, &text(&line), &cell);
    assert!(result.is_err());
    let report = result.unwrap_err();
    assert!(matches!(
        report.current_context(),
        CmdError::BuiltinKeywordNotSupported { .. }
    ));
}

#[test]
fn run_read_captures_not_supported() {
    let line = make_read_line(&["read", "var1"]);
    let cmdline = make_read_cmdline(&[c"var1".into()]);
    let mut cmdline = cmdline;
    cmdline.captures = vec![Capture {
        var: c"fd".into(),
        tag: None,
        force: false,
    }];
    let cell = make_read_cell();
    let result = run_read(&line, &cmdline, &text(&line), &cell);
    assert!(result.is_err());
    let report = result.unwrap_err();
    assert!(matches!(
        report.current_context(),
        CmdError::CapturesNotSupported { .. }
    ));
}

#[test]
fn run_read_redirects_not_supported() {
    let line = make_read_line(&["read", "var1"]);
    let cmdline = make_read_cmdline(&[c"var1".into()]);
    let mut cmdline = cmdline;
    cmdline.redirects = vec![RedirectDef {
        export_to: 1,
        direction: RedirectDirection::Write,
        source: RedirectSource::Var(c"test".into()),
    }];
    let cell = make_read_cell();
    let result = run_read(&line, &cmdline, &text(&line), &cell);
    assert!(result.is_err());
    let report = result.unwrap_err();
    assert!(matches!(
        report.current_context(),
        CmdError::RedirectNotSupported { .. }
    ));
}

#[test]
fn run_read_with_prompt() {
    let (read_end, write_end) = sys::pipe::pipe2(0).unwrap();
    let data = b"answer\n";
    sys::rw::write(&write_end, data).unwrap();
    drop(write_end);

    let exported = read_end.export().unwrap();
    let fd = exported.as_raw();
    let line = make_read_u_line(&[c"-p".into(), c"Enter: ".into(), c"var1".into()], fd);
    let cmdline = make_read_u_cmdline(&[c"-p".into(), c"Enter: ".into(), c"var1".into()], fd);
    let cell = make_read_cell();

    let result = run_read(&line, &cmdline, &text(&line), &cell);
    assert!(result.is_ok());
    assert!(result.unwrap());

    let state = cell.borrow().unwrap();
    assert_eq!(
        state
            .strings
            .get::<ShortCStr>(&c"var1".into())
            .map(|v| &v.value),
        Some(&c"answer".into())
    );
}

#[test]
fn run_read_data_without_newline_sets_vars() {
    let (read_end, write_end) = sys::pipe::pipe2(0).unwrap();
    sys::rw::write(&write_end, b"hello").unwrap();
    drop(write_end);

    let exported = read_end.export().unwrap();
    let fd = exported.as_raw();
    let line = make_read_u_line(&[c"var1".into()], fd);
    let cmdline = make_read_u_cmdline(&[c"var1".into()], fd);
    let cell = make_read_cell();

    let result = run_read(&line, &cmdline, &text(&line), &cell);
    assert!(result.is_ok());
    assert!(result.unwrap());

    let state = cell.borrow().unwrap();
    assert_eq!(
        state
            .strings
            .get::<ShortCStr>(&c"var1".into())
            .map(|v| &v.value),
        Some(&c"hello".into())
    );
    assert!(matches!(state.last_status, WaitStatus::Exited(0)));
}

#[test]
fn run_read_writes_prompt_to_stderr() {
    let (data_r, data_w) = sys::pipe::pipe2(0).unwrap();
    sys::rw::write(&data_w, b"answer\n").unwrap();
    drop(data_w);

    let exported = data_r.export().unwrap();
    let fd = exported.as_raw();
    let line = make_read_u_line(&[c"-p".into(), c"Enter: ".into(), c"var1".into()], fd);
    let cmdline = make_read_u_cmdline(&[c"-p".into(), c"Enter: ".into(), c"var1".into()], fd);
    let cell = make_read_cell();

    let (err_r, err_w) = sys::pipe::pipe2(0).unwrap();
    match sys::fork_pidfd::fork_pidfd().unwrap().1 {
        None => {
            err_w.export_to(2).unwrap();
            drop(err_w);
            let result = run_read(&line, &cmdline, &text(&line), &cell);
            sys::exit(if result.is_ok() { 0 } else { 1 });
        }
        Some(pidfd) => {
            drop(err_w);
            let mut err = Vec::new();
            let mut chunk = [0u8; 4096];
            loop {
                let n = err_r.read(&mut chunk).unwrap();
                if n == 0 {
                    break;
                }
                if let Some(part) = chunk.get(..n) {
                    err.extend_from_slice(part);
                }
            }
            match sys::wait_pidfd::wait_pidfd(&pidfd).unwrap() {
                WaitStatus::Exited(0) => {}
                other => panic!("child failed: {}", other.exit_code()),
            }
            assert_eq!(err, b"Enter: ");
        }
    }
}

#[test]
fn run_read_with_n_max_bytes() {
    let (read_end, write_end) = sys::pipe::pipe2(0).unwrap();
    let data = b"hello world\n";
    sys::rw::write(&write_end, data).unwrap();
    drop(write_end);

    let exported = read_end.export().unwrap();
    let fd = exported.as_raw();
    let line = make_read_u_line(&[c"-n".into(), c"3".into(), c"var1".into()], fd);
    let cmdline = make_read_u_cmdline(&[c"-n".into(), c"3".into(), c"var1".into()], fd);
    let cell = make_read_cell();

    let result = run_read(&line, &cmdline, &text(&line), &cell);
    assert!(result.is_ok());
    assert!(result.unwrap());

    let state = cell.borrow().unwrap();
    assert_eq!(
        state
            .strings
            .get::<ShortCStr>(&c"var1".into())
            .map(|v| &v.value),
        Some(&c"hel".into())
    );
}

#[test]
fn run_read_with_u_fdvar() {
    let (read_end, write_end) = sys::pipe::pipe2(0).unwrap();
    let data = b"from var\n";
    sys::rw::write(&write_end, data).unwrap();
    drop(write_end);

    let cell = make_read_cell();
    {
        let mut state = cell.borrow_mut().unwrap();
        state.fds.insert(c"MYFD".into(), read_end);
    }

    let line = make_read_line(&["read", "-u", "%MYFD", "var1"]);
    let cmdline = make_read_cmdline(&[c"-u".into(), c"%MYFD".into(), c"var1".into()]);
    let result = run_read(&line, &cmdline, &text(&line), &cell);
    assert!(result.is_ok());
    assert!(result.unwrap());

    let state = cell.borrow().unwrap();
    assert_eq!(
        state
            .strings
            .get::<ShortCStr>(&c"var1".into())
            .map(|v| &v.value),
        Some(&c"from var".into())
    );
}

#[test]
fn run_read_with_u_fdvar_not_found() {
    let line = make_read_line(&["read", "-u", "%NONEXISTENT", "var1"]);
    let cmdline = make_read_cmdline(&[c"-u".into(), c"%NONEXISTENT".into(), c"var1".into()]);
    let cell = make_read_cell();
    let result = run_read(&line, &cmdline, &text(&line), &cell);
    assert!(result.is_err());
    let report = result.unwrap_err();
    assert!(matches!(report.current_context(), CmdError::Read));
}

#[test]
fn run_read_nul_byte_error() {
    let (read_end, write_end) = sys::pipe::pipe2(0).unwrap();
    let data = b"a\0b\n";
    sys::rw::write(&write_end, data).unwrap();
    drop(write_end);

    let exported = read_end.export().unwrap();
    let fd = exported.as_raw();
    let line = make_read_u_line(&[c"var1".into()], fd);
    let cmdline = make_read_u_cmdline(&[c"var1".into()], fd);
    let cell = make_read_cell();

    let result = run_read(&line, &cmdline, &text(&line), &cell);
    assert!(result.is_err());
    let report = result.unwrap_err();
    assert!(matches!(report.current_context(), CmdError::Read));
}

#[test]
fn run_read_multiple_targets() {
    let (read_end, write_end) = sys::pipe::pipe2(0).unwrap();
    let data = b"a b c\n";
    sys::rw::write(&write_end, data).unwrap();
    drop(write_end);

    let exported = read_end.export().unwrap();
    let fd = exported.as_raw();
    let line = make_read_u_line(&[c"x".into(), c"y".into(), c"z".into()], fd);
    let cmdline = make_read_u_cmdline(&[c"x".into(), c"y".into(), c"z".into()], fd);
    let cell = make_read_cell();

    let result = run_read(&line, &cmdline, &text(&line), &cell);
    assert!(result.is_ok());
    assert!(result.unwrap());

    let state = cell.borrow().unwrap();
    assert_eq!(
        state
            .strings
            .get::<ShortCStr>(&c"x".into())
            .map(|v| &v.value),
        Some(&c"a".into())
    );
    assert_eq!(
        state
            .strings
            .get::<ShortCStr>(&c"y".into())
            .map(|v| &v.value),
        Some(&c"b".into())
    );
    assert_eq!(
        state
            .strings
            .get::<ShortCStr>(&c"z".into())
            .map(|v| &v.value),
        Some(&c"c".into())
    );
}

#[test]
fn run_read_fewer_fields_than_targets() {
    let (read_end, write_end) = sys::pipe::pipe2(0).unwrap();
    let data = b"only_one\n";
    sys::rw::write(&write_end, data).unwrap();
    drop(write_end);

    let exported = read_end.export().unwrap();
    let fd = exported.as_raw();
    let line = make_read_u_line(&[c"x".into(), c"y".into(), c"z".into()], fd);
    let cmdline = make_read_u_cmdline(&[c"x".into(), c"y".into(), c"z".into()], fd);
    let cell = make_read_cell();

    let result = run_read(&line, &cmdline, &text(&line), &cell);
    assert!(result.is_ok());
    assert!(result.unwrap());

    let state = cell.borrow().unwrap();
    assert_eq!(
        state
            .strings
            .get::<ShortCStr>(&c"x".into())
            .map(|v| &v.value),
        Some(&c"only_one".into())
    );
    assert_eq!(
        state
            .strings
            .get::<ShortCStr>(&c"y".into())
            .map(|v| &v.value),
        Some(&ShortCStr::new())
    );
    assert_eq!(
        state
            .strings
            .get::<ShortCStr>(&c"z".into())
            .map(|v| &v.value),
        Some(&ShortCStr::new())
    );
}

#[test]
fn run_read_more_fields_than_targets() {
    let (read_end, write_end) = sys::pipe::pipe2(0).unwrap();
    let data = b"a b c d\n";
    sys::rw::write(&write_end, data).unwrap();
    drop(write_end);

    let exported = read_end.export().unwrap();
    let fd = exported.as_raw();
    let line = make_read_u_line(&[c"x".into()], fd);
    let cmdline = make_read_u_cmdline(&[c"x".into()], fd);
    let cell = make_read_cell();

    let result = run_read(&line, &cmdline, &text(&line), &cell);
    assert!(result.is_ok());
    assert!(result.unwrap());

    let state = cell.borrow().unwrap();
    assert_eq!(
        state
            .strings
            .get::<ShortCStr>(&c"x".into())
            .map(|v| &v.value),
        Some(&c"a b c d".into())
    );
}

#[test]
fn run_read_status_0_on_success() {
    let (read_end, write_end) = sys::pipe::pipe2(0).unwrap();
    let data = b"hello\n";
    sys::rw::write(&write_end, data).unwrap();
    drop(write_end);

    let exported = read_end.export().unwrap();
    let fd = exported.as_raw();
    let line = make_read_u_line(&[c"var1".into()], fd);
    let cmdline = make_read_u_cmdline(&[c"var1".into()], fd);
    let cell = make_read_cell();

    let result = run_read(&line, &cmdline, &text(&line), &cell);
    assert!(result.is_ok());
    assert!(result.unwrap());

    let state = cell.borrow().unwrap();
    assert!(matches!(state.last_status, WaitStatus::Exited(0)));
}

#[test]
fn run_read_strip_prefix_dollar() {
    let (read_end, write_end) = sys::pipe::pipe2(0).unwrap();
    let data = b"value\n";
    sys::rw::write(&write_end, data).unwrap();
    drop(write_end);

    let exported = read_end.export().unwrap();
    let fd = exported.as_raw();
    let line = make_read_u_line(&[c"$MYVAR".into()], fd);
    let cmdline = make_read_u_cmdline(&[c"$MYVAR".into()], fd);
    let cell = make_read_cell();

    let result = run_read(&line, &cmdline, &text(&line), &cell);
    assert!(result.is_ok());
    assert!(result.unwrap());

    let state = cell.borrow().unwrap();
    assert_eq!(
        state
            .strings
            .get::<ShortCStr>(&c"MYVAR".into())
            .map(|v| &v.value),
        Some(&c"value".into())
    );
}

#[test]
fn run_read_empty_data_eof() {
    let (read_end, write_end) = sys::pipe::pipe2(0).unwrap();
    drop(write_end);

    let exported = read_end.export().unwrap();
    let fd = exported.as_raw();
    let line = make_read_u_line(&[c"var1".into()], fd);
    let cmdline = make_read_u_cmdline(&[c"var1".into()], fd);
    let cell = make_read_cell();

    let result = run_read(&line, &cmdline, &text(&line), &cell);
    assert!(result.is_ok());
    assert!(result.unwrap());

    let state = cell.borrow().unwrap();
    assert!(matches!(state.last_status, WaitStatus::Exited(1)));
    assert!(
        !state
            .strings
            .contains_key::<sys::ShortCStr>(&c"var1".into())
    );
}

#[test]
fn run_read_newline_stops_reading() {
    let (read_end, write_end) = sys::pipe::pipe2(0).unwrap();
    let data = b"first\nsecond\n";
    sys::rw::write(&write_end, data).unwrap();
    drop(write_end);

    let exported = read_end.export().unwrap();
    let fd = exported.as_raw();
    let line = make_read_u_line(&[c"var1".into()], fd);
    let cmdline = make_read_u_cmdline(&[c"var1".into()], fd);
    let cell = make_read_cell();

    let result = run_read(&line, &cmdline, &text(&line), &cell);
    assert!(result.is_ok());
    assert!(result.unwrap());

    let state = cell.borrow().unwrap();
    assert_eq!(
        state
            .strings
            .get::<ShortCStr>(&c"var1".into())
            .map(|v| &v.value),
        Some(&c"first".into())
    );
}
