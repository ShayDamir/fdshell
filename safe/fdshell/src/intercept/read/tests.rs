use super::read_from_fd::read_from_local_fd;
use super::*;

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
