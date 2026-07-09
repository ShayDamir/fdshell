use super::read_from_fd::read_from_local_fd;
use super::*;
use crate::capture::Capture;
use crate::parse::CommandLine;
use crate::redirect::{RedirectDef, RedirectDirection, RedirectSource};
use sys::ShortCStr;

fn make_read_cmdline(args: &[&str]) -> CommandLine {
    let args_vec: Vec<ShortCStr> = args
        .iter()
        .map(|s| ShortCStr::from_vec(s.as_bytes().to_vec()).unwrap())
        .collect();
    CommandLine {
        builtin: false,
        command: ShortCStr::from_vec(b"read".to_vec()).unwrap(),
        args: args_vec,
        args_fq: vec![false; args.len()],
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
fn test_strip_prefix_dollar() {
    let name = c"$FOO".into();
    assert_eq!(strip_prefix(&name), c"FOO".into());
}

#[test]
fn test_strip_prefix_bare() {
    let name = c"FOO".into();
    assert_eq!(strip_prefix(&name), c"FOO".into());
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
    assert!(matches!(source, SourceFd::RawFd(3)));
}

#[test]
fn test_parse_flags_u_negative() {
    let args = vec![c"-u".into(), c"-1".into()];
    let (source, _, _) = parse_flags(&args).unwrap();
    assert!(matches!(source, SourceFd::RawFd(-1)));
}

#[test]
fn test_parse_flags_u_fdvar() {
    let args = vec![c"-u".into(), c"%MYVAR".into()];
    let (source, _, _) = parse_flags(&args).unwrap();
    assert!(matches!(source, SourceFd::FdVar(ref v) if v == b"MYVAR"));
}

#[test]
fn test_parse_flags_u_invalid() {
    let args = vec![c"-u".into(), c"abc".into()];
    let result = parse_flags(&args);
    assert!(result.is_err());
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
    assert!(matches!(source, SourceFd::RawFd(3)));
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
    assert!(matches!(source, SourceFd::RawFd(5)));
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
fn test_read_from_local_fd_eof() {
    let (read_end, write_end) = sys::pipe::pipe2(0).unwrap();
    // Close write end immediately → EOF
    write_end.try_close().unwrap();

    let mut buf = Vec::new();
    let mut eof = false;
    read_from_local_fd(&read_end, &mut buf, &mut eof, None).unwrap();
    assert!(eof);
    assert!(buf.is_empty());
}

#[test]
fn test_read_from_local_fd_max_bytes() {
    let (read_end, write_end) = sys::pipe::pipe2(0).unwrap();
    let data = b"hello world";
    sys::rw::write(&write_end, data).unwrap();
    write_end.try_close().unwrap();

    let mut buf = Vec::new();
    let mut eof = false;
    read_from_local_fd(&read_end, &mut buf, &mut eof, Some(5)).unwrap();
    assert_eq!(buf, b"hello");
}

#[test]
fn test_read_from_local_fd_stops_at_newline() {
    let (read_end, write_end) = sys::pipe::pipe2(0).unwrap();
    let data = b"line1\nline2";
    sys::rw::write(&write_end, data).unwrap();
    write_end.try_close().unwrap();

    let mut buf = Vec::new();
    let mut eof = false;
    read_from_local_fd(&read_end, &mut buf, &mut eof, None).unwrap();
    assert_eq!(buf, b"line1");
}

// read_line tests via SourceFd::RawFd

#[test]
fn test_read_line_rawfd_eof() {
    let (read_end, write_end) = sys::pipe::pipe2(0).unwrap();
    write_end.try_close().unwrap();

    let source = SourceFd::RawFd(read_end.as_raw());
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
    write_end.try_close().unwrap();

    let source = SourceFd::RawFd(read_end.as_raw());
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
    write_end.try_close().unwrap();

    let source = SourceFd::RawFd(read_end.as_raw());
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
    write_end.try_close().unwrap();

    let source = SourceFd::RawFd(read_end.as_raw());
    let result = read_line(&source, None, None);
    assert!(result.is_ok());
    let (buf, eof) = result.unwrap();
    assert!(!eof);
    assert_eq!(buf, b"first");
}

#[test]
fn test_read_line_fdvar_no_clone() {
    let source = SourceFd::FdVar(b"MYVAR".to_vec());
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
    write_end.try_close().unwrap();

    let source = SourceFd::FdVar(b"MYVAR".to_vec());
    let result = read_line(&source, Some(&read_end), None);
    assert!(result.is_ok());
    let (buf, eof) = result.unwrap();
    assert!(!eof);
    assert_eq!(buf, b"from var");
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

// strip.rs edge cases

#[test]
fn test_strip_prefix_empty() {
    let name = ShortCStr::new();
    let result = strip_prefix(&name);
    assert_eq!(result.as_bytes().unwrap(), b"");
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

fn make_read_u_cmdline(args: &[&str], fd: i32) -> CommandLine {
    let fd_str = fd.to_string();
    let mut all: Vec<&str> = vec!["-u", &fd_str];
    all.extend(args);
    make_read_cmdline(&all)
}

fn make_read_u_line(args: &[&str], fd: i32) -> Vec<u8> {
    let fd_str = fd.to_string();
    let mut all: Vec<&str> = vec!["read", "-u", &fd_str];
    all.extend(args);
    all.join(" ").into_bytes()
}

#[test]
fn run_read_simple() {
    let (read_end, write_end) = sys::pipe::pipe2(0).unwrap();
    let data = b"hello world\n";
    sys::rw::write(&write_end, data).unwrap();
    write_end.try_close().unwrap();

    let fd = read_end.as_raw();
    let line = make_read_u_line(&["var1", "var2"], fd);
    let cmdline = make_read_u_cmdline(&["var1", "var2"], fd);
    let cell = make_read_cell();

    let result = run_read(&line, &cmdline, &cell);
    assert!(result.is_ok());
    assert!(result.unwrap());

    let state = cell.borrow().unwrap();
    assert_eq!(
        state.strings.get(&c"var1".into()),
        Some(&ShortCStr::from_vec(b"hello".to_vec()).unwrap())
    );
    assert_eq!(
        state.strings.get(&c"var2".into()),
        Some(&ShortCStr::from_vec(b"world".to_vec()).unwrap())
    );
}

#[test]
fn run_read_eof_returns_status_1() {
    let (read_end, write_end) = sys::pipe::pipe2(0).unwrap();
    write_end.try_close().unwrap();

    let fd = read_end.as_raw();
    let line = make_read_u_line(&["var1"], fd);
    let cmdline = make_read_u_cmdline(&["var1"], fd);
    let cell = make_read_cell();

    let result = run_read(&line, &cmdline, &cell);
    assert!(result.is_ok());
    assert!(result.unwrap());

    let state = cell.borrow().unwrap();
    assert!(matches!(state.last_status, WaitStatus::Exited(1)));
}

#[test]
fn run_read_builtin_not_supported() {
    let line = make_read_line(&["builtin", "read", "var1"]);
    let cmdline = make_read_cmdline(&["var1"]);
    let mut cmdline = cmdline;
    cmdline.builtin = true;
    let cell = make_read_cell();
    let result = run_read(&line, &cmdline, &cell);
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
    let cmdline = make_read_cmdline(&["var1"]);
    let mut cmdline = cmdline;
    cmdline.captures = vec![Capture {
        var: ShortCStr::from_vec(b"fd".to_vec()).unwrap(),
        tag: None,
        force: false,
    }];
    let cell = make_read_cell();
    let result = run_read(&line, &cmdline, &cell);
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
    let cmdline = make_read_cmdline(&["var1"]);
    let mut cmdline = cmdline;
    cmdline.redirects = vec![RedirectDef {
        export_to: 1,
        direction: RedirectDirection::Write,
        source: RedirectSource::Var(ShortCStr::from_vec(b"test".to_vec()).unwrap()),
    }];
    let cell = make_read_cell();
    let result = run_read(&line, &cmdline, &cell);
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
    write_end.try_close().unwrap();

    let fd = read_end.as_raw();
    let line = make_read_u_line(&["-p", "Enter: ", "var1"], fd);
    let cmdline = make_read_u_cmdline(&["-p", "Enter: ", "var1"], fd);
    let cell = make_read_cell();

    let result = run_read(&line, &cmdline, &cell);
    assert!(result.is_ok());
    assert!(result.unwrap());

    let state = cell.borrow().unwrap();
    assert_eq!(
        state.strings.get(&c"var1".into()),
        Some(&ShortCStr::from_vec(b"answer".to_vec()).unwrap())
    );
}

#[test]
fn run_read_with_n_max_bytes() {
    let (read_end, write_end) = sys::pipe::pipe2(0).unwrap();
    let data = b"hello world\n";
    sys::rw::write(&write_end, data).unwrap();
    write_end.try_close().unwrap();

    let fd = read_end.as_raw();
    let line = make_read_u_line(&["-n", "3", "var1"], fd);
    let cmdline = make_read_u_cmdline(&["-n", "3", "var1"], fd);
    let cell = make_read_cell();

    let result = run_read(&line, &cmdline, &cell);
    assert!(result.is_ok());
    assert!(result.unwrap());

    let state = cell.borrow().unwrap();
    assert_eq!(
        state.strings.get(&c"var1".into()),
        Some(&ShortCStr::from_vec(b"hel".to_vec()).unwrap())
    );
}

#[test]
fn run_read_with_u_fdvar() {
    let (read_end, write_end) = sys::pipe::pipe2(0).unwrap();
    let data = b"from var\n";
    sys::rw::write(&write_end, data).unwrap();
    write_end.try_close().unwrap();

    let cell = make_read_cell();
    {
        let mut state = cell.borrow_mut().unwrap();
        state.fds.insert(c"MYFD".into(), read_end);
    }

    let line = make_read_line(&["read", "-u", "%MYFD", "var1"]);
    let cmdline = make_read_cmdline(&["-u", "%MYFD", "var1"]);
    let result = run_read(&line, &cmdline, &cell);
    assert!(result.is_ok());
    assert!(result.unwrap());

    let state = cell.borrow().unwrap();
    assert_eq!(
        state.strings.get(&c"var1".into()),
        Some(&ShortCStr::from_vec(b"from var".to_vec()).unwrap())
    );
}

#[test]
fn run_read_with_u_fdvar_not_found() {
    let line = make_read_line(&["read", "-u", "%NONEXISTENT", "var1"]);
    let cmdline = make_read_cmdline(&["-u", "%NONEXISTENT", "var1"]);
    let cell = make_read_cell();
    let result = run_read(&line, &cmdline, &cell);
    assert!(result.is_err());
    let report = result.unwrap_err();
    assert!(matches!(report.current_context(), CmdError::Read));
}

#[test]
fn run_read_multiple_targets() {
    let (read_end, write_end) = sys::pipe::pipe2(0).unwrap();
    let data = b"a b c\n";
    sys::rw::write(&write_end, data).unwrap();
    write_end.try_close().unwrap();

    let fd = read_end.as_raw();
    let line = make_read_u_line(&["x", "y", "z"], fd);
    let cmdline = make_read_u_cmdline(&["x", "y", "z"], fd);
    let cell = make_read_cell();

    let result = run_read(&line, &cmdline, &cell);
    assert!(result.is_ok());
    assert!(result.unwrap());

    let state = cell.borrow().unwrap();
    assert_eq!(
        state.strings.get(&c"x".into()),
        Some(&ShortCStr::from_vec(b"a".to_vec()).unwrap())
    );
    assert_eq!(
        state.strings.get(&c"y".into()),
        Some(&ShortCStr::from_vec(b"b".to_vec()).unwrap())
    );
    assert_eq!(
        state.strings.get(&c"z".into()),
        Some(&ShortCStr::from_vec(b"c".to_vec()).unwrap())
    );
}

#[test]
fn run_read_fewer_fields_than_targets() {
    let (read_end, write_end) = sys::pipe::pipe2(0).unwrap();
    let data = b"only_one\n";
    sys::rw::write(&write_end, data).unwrap();
    write_end.try_close().unwrap();

    let fd = read_end.as_raw();
    let line = make_read_u_line(&["x", "y", "z"], fd);
    let cmdline = make_read_u_cmdline(&["x", "y", "z"], fd);
    let cell = make_read_cell();

    let result = run_read(&line, &cmdline, &cell);
    assert!(result.is_ok());
    assert!(result.unwrap());

    let state = cell.borrow().unwrap();
    assert_eq!(
        state.strings.get(&c"x".into()),
        Some(&ShortCStr::from_vec(b"only_one".to_vec()).unwrap())
    );
    assert_eq!(
        state.strings.get(&c"y".into()),
        Some(&ShortCStr::from_vec(b"".to_vec()).unwrap())
    );
    assert_eq!(
        state.strings.get(&c"z".into()),
        Some(&ShortCStr::from_vec(b"".to_vec()).unwrap())
    );
}

#[test]
fn run_read_more_fields_than_targets() {
    let (read_end, write_end) = sys::pipe::pipe2(0).unwrap();
    let data = b"a b c d\n";
    sys::rw::write(&write_end, data).unwrap();
    write_end.try_close().unwrap();

    let fd = read_end.as_raw();
    let line = make_read_u_line(&["x"], fd);
    let cmdline = make_read_u_cmdline(&["x"], fd);
    let cell = make_read_cell();

    let result = run_read(&line, &cmdline, &cell);
    assert!(result.is_ok());
    assert!(result.unwrap());

    let state = cell.borrow().unwrap();
    assert_eq!(
        state.strings.get(&c"x".into()),
        Some(&ShortCStr::from_vec(b"a b c d".to_vec()).unwrap())
    );
}

#[test]
fn run_read_status_0_on_success() {
    let (read_end, write_end) = sys::pipe::pipe2(0).unwrap();
    let data = b"hello\n";
    sys::rw::write(&write_end, data).unwrap();
    write_end.try_close().unwrap();

    let fd = read_end.as_raw();
    let line = make_read_u_line(&["var1"], fd);
    let cmdline = make_read_u_cmdline(&["var1"], fd);
    let cell = make_read_cell();

    let result = run_read(&line, &cmdline, &cell);
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
    write_end.try_close().unwrap();

    let fd = read_end.as_raw();
    let line = make_read_u_line(&["$MYVAR"], fd);
    let cmdline = make_read_u_cmdline(&["$MYVAR"], fd);
    let cell = make_read_cell();

    let result = run_read(&line, &cmdline, &cell);
    assert!(result.is_ok());
    assert!(result.unwrap());

    let state = cell.borrow().unwrap();
    assert_eq!(
        state.strings.get(&c"MYVAR".into()),
        Some(&ShortCStr::from_vec(b"value".to_vec()).unwrap())
    );
}

#[test]
fn run_read_empty_data_eof() {
    let (read_end, write_end) = sys::pipe::pipe2(0).unwrap();
    write_end.try_close().unwrap();

    let fd = read_end.as_raw();
    let line = make_read_u_line(&["var1"], fd);
    let cmdline = make_read_u_cmdline(&["var1"], fd);
    let cell = make_read_cell();

    let result = run_read(&line, &cmdline, &cell);
    assert!(result.is_ok());
    assert!(result.unwrap());

    let state = cell.borrow().unwrap();
    assert!(matches!(state.last_status, WaitStatus::Exited(1)));
    assert!(!state.strings.contains_key(&c"var1".into()));
}

#[test]
fn run_read_newline_stops_reading() {
    let (read_end, write_end) = sys::pipe::pipe2(0).unwrap();
    let data = b"first\nsecond\n";
    sys::rw::write(&write_end, data).unwrap();
    write_end.try_close().unwrap();

    let fd = read_end.as_raw();
    let line = make_read_u_line(&["var1"], fd);
    let cmdline = make_read_u_cmdline(&["var1"], fd);
    let cell = make_read_cell();

    let result = run_read(&line, &cmdline, &cell);
    assert!(result.is_ok());
    assert!(result.unwrap());

    let state = cell.borrow().unwrap();
    assert_eq!(
        state.strings.get(&c"var1".into()),
        Some(&ShortCStr::from_vec(b"first".to_vec()).unwrap())
    );
}
