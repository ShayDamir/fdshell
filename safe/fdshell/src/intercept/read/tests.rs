use super::flags::ReadFlags;
use super::line::{Line, LineEnd};
use super::read_from_fd::read_line_from_fd;
use super::*;
use crate::capture::Capture;
use crate::parse::{BuiltinPrefix, CommandLine};
use crate::redirect::{RedirectDef, RedirectDirection, RedirectSource};
use alloc::format;
use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;
use sys::ShortCStr;
use sys::SyscallError;
use sys::siginfo::WaitStatus;

fn make_flags(
    delim: Option<&'static [u8]>,
    raw: bool,
    max: Option<usize>,
    timeout: Option<u32>,
) -> ReadFlags<'static> {
    ReadFlags {
        source: SourceFd::Stdin,
        max_bytes: max,
        prompt: None,
        raw,
        delim,
        timeout,
    }
}

fn make_read_cmdline(args: &[ShortCStr]) -> CommandLine {
    CommandLine {
        prefix: BuiltinPrefix::None,
        command: c"read".into(),
        command_mask: vec![],
        env_assigns: vec![],
        args_mask: vec![vec![]; args.len()],
        args_quoted: vec![false; args.len()],
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
    let flags = parse_flags(&args).unwrap();
    assert!(matches!(flags.source, SourceFd::Stdin));
    assert!(flags.max_bytes.is_none());
    assert!(flags.prompt.is_none());
    assert!(!flags.raw);
    assert!(flags.delim.is_none());
    assert!(flags.timeout.is_none());
}

#[test]
fn test_parse_flags_u_numeric() {
    let args = vec![c"-u".into(), c"3".into()];
    let flags = parse_flags(&args).unwrap();
    assert!(matches!(flags.source, SourceFd::RawFd(_)));
}

#[test]
fn test_parse_flags_u_negative() {
    let args = vec![c"-u".into(), c"-1".into()];
    let flags = parse_flags(&args).unwrap();
    assert!(matches!(flags.source, SourceFd::RawFd(_)));
}

#[test]
fn test_parse_flags_u_fdvar() {
    let args = vec![c"-u".into(), c"%MYVAR".into()];
    let flags = parse_flags(&args).unwrap();
    assert!(matches!(flags.source, SourceFd::FdVar(v) if v.as_bytes().unwrap() == b"MYVAR"));
}

#[test]
fn test_parse_flags_u_invalid() {
    let args = vec![c"-u".into(), c"abc".into()];
    let flags = parse_flags(&args).unwrap();
    assert!(matches!(flags.source, SourceFd::RawFd(_)));
}

#[test]
fn test_parse_flags_n_positive() {
    let args = vec![c"-n".into(), c"10".into()];
    let flags = parse_flags(&args).unwrap();
    assert_eq!(flags.max_bytes, Some(10));
}

#[test]
fn test_parse_flags_n_zero() {
    let args = vec![c"-n".into(), c"0".into()];
    let flags = parse_flags(&args).unwrap();
    assert_eq!(flags.max_bytes, Some(0));
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
    let flags = parse_flags(&args).unwrap();
    assert_eq!(flags.prompt, Some(b"Enter: " as &[u8]));
}

#[test]
fn test_parse_flags_r_raw() {
    let args = vec![c"-r".into(), c"x".into()];
    let flags = parse_flags(&args).unwrap();
    assert!(flags.raw);
}

#[test]
fn test_parse_flags_d_delim() {
    let args = vec![c"-d".into(), c":".into()];
    let flags = parse_flags(&args).unwrap();
    assert_eq!(flags.delim, Some(b":" as &[u8]));
}

#[test]
fn test_parse_flags_d_empty_delim() {
    let args = vec![c"-d".into(), c"".into()];
    let flags = parse_flags(&args).unwrap();
    assert_eq!(flags.delim, Some(b"" as &[u8]));
}

#[test]
fn test_parse_flags_t_seconds() {
    let args = vec![c"-t".into(), c"5".into()];
    let flags = parse_flags(&args).unwrap();
    assert_eq!(flags.timeout, Some(5));
    assert_eq!(flags.timeout_ms(), Some(5000));
}

#[test]
fn test_parse_flags_t_zero() {
    let args = vec![c"-t".into(), c"0".into()];
    let flags = parse_flags(&args).unwrap();
    assert_eq!(flags.timeout, Some(0));
    assert_eq!(flags.timeout_ms(), Some(0));
}

#[test]
fn test_parse_flags_t_overflow_saturates_infinite() {
    let args = vec![c"-t".into(), c"4294967295".into()];
    let flags = parse_flags(&args).unwrap();
    assert_eq!(flags.timeout_ms(), Some(-1));
}

#[test]
fn test_parse_flags_t_invalid() {
    let args = vec![c"-t".into(), c"abc".into()];
    let result = parse_flags(&args);
    assert!(result.is_err());
}

#[test]
fn test_parse_flags_t_negative() {
    let args = vec![c"-t".into(), c"-1".into()];
    let result = parse_flags(&args);
    assert!(result.is_err());
}

#[test]
fn test_parse_flags_t_fraction_rejected() {
    let args = vec![c"-t".into(), c"0.5".into()];
    let result = parse_flags(&args);
    assert!(result.is_err());
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
        c"-r".into(),
        c"-d".into(),
        c":".into(),
        c"-t".into(),
        c"2".into(),
    ];
    let flags = parse_flags(&args).unwrap();
    assert!(matches!(flags.source, SourceFd::RawFd(_)));
    assert_eq!(flags.max_bytes, Some(5));
    assert_eq!(flags.prompt, Some(b"hi" as &[u8]));
    assert!(flags.raw);
    assert_eq!(flags.delim, Some(b":" as &[u8]));
    assert_eq!(flags.timeout, Some(2));
}

#[test]
fn test_parse_flags_d_last_wins() {
    let args = vec![c"-d".into(), c":".into(), c"-d".into(), c"|".into()];
    let flags = parse_flags(&args).unwrap();
    assert_eq!(flags.delim, Some(b"|" as &[u8]));
}

#[test]
fn test_parse_flags_t_last_wins() {
    let args = vec![c"-t".into(), c"5".into(), c"-t".into(), c"2".into()];
    let flags = parse_flags(&args).unwrap();
    assert_eq!(flags.timeout, Some(2));
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
fn test_parse_flags_d_missing_arg() {
    let args = vec![c"-d".into()];
    let result = parse_flags(&args);
    assert!(result.is_err());
}

#[test]
fn test_parse_flags_t_missing_arg() {
    let args = vec![c"-t".into()];
    let result = parse_flags(&args);
    assert!(result.is_err());
}

#[test]
fn test_parse_flags_unknown_arg_ignored() {
    let args = vec![c"-x".into(), c"value".into()];
    let flags = parse_flags(&args).unwrap();
    assert!(matches!(flags.source, SourceFd::Stdin));
    assert!(flags.max_bytes.is_none());
    assert!(flags.prompt.is_none());
    assert!(!flags.raw);
    assert!(flags.delim.is_none());
    assert!(flags.timeout.is_none());
}

#[test]
fn test_parse_flags_multiple_u_last_wins() {
    let args = vec![c"-u".into(), c"3".into(), c"-u".into(), c"5".into()];
    let flags = parse_flags(&args).unwrap();
    assert!(matches!(flags.source, SourceFd::RawFd(_)));
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

#[test]
fn test_collect_targets_r_consumes_nothing() {
    let args = vec![c"-r".into(), c"x".into()];
    let targets = collect_targets(&args).unwrap();
    assert_eq!(targets, vec![c"x".into()]);
}

#[test]
fn test_collect_targets_d_t_args_consumed() {
    let args = vec![
        c"-d".into(),
        c":".into(),
        c"-t".into(),
        c"5".into(),
        c"x".into(),
    ];
    let targets = collect_targets(&args).unwrap();
    assert_eq!(targets, vec![c"x".into()]);
}

// Line accumulator tests (pure, no fds)

#[test]
fn test_line_default_newline_delimiter() {
    let mut line = Line::new(None, false, None);
    for &b in b"abc" {
        line.feed(b);
    }
    line.feed(b'\n');
    assert!(line.finished());
    assert_eq!(line.buf, b"abc");
    assert_eq!(line.end(), LineEnd::Delim);
}

#[test]
fn test_line_backslash_newline_continuation_dropped() {
    let mut line = Line::new(None, false, None);
    // `a` + backslash + newline + `b`: the continuation pair is dropped.
    for &b in b"a\\\nb" {
        line.feed(b);
    }
    line.end_eof();
    assert_eq!(line.buf, b"ab");
    assert_eq!(line.end(), LineEnd::Eof);
}

#[test]
fn test_line_backslash_drops_backslash_keeps_byte() {
    let mut line = Line::new(None, false, None);
    for &b in b"a\\:b" {
        line.feed(b);
    }
    line.end_eof();
    assert_eq!(line.buf, b"a:b");
}

#[test]
fn test_line_trailing_backslash_at_eof_dropped() {
    let mut line = Line::new(None, false, None);
    for &b in b"a\\" {
        line.feed(b);
    }
    line.end_eof();
    // bash: a backslash at EOF is dropped, not kept.
    assert_eq!(line.buf, b"a");
    assert_eq!(line.end(), LineEnd::Eof);
}

#[test]
fn test_line_raw_keeps_all_backslashes() {
    let mut line = Line::new(None, true, None);
    for &b in b"a\\nb" {
        line.feed(b);
    }
    line.end_eof();
    assert_eq!(line.buf, b"a\\nb");
}

#[test]
fn test_line_single_char_delimiter() {
    let mut line = Line::new(Some(b":"), false, None);
    for &b in b"a:b:c" {
        line.feed(b);
    }
    assert!(line.finished());
    assert_eq!(line.buf, b"a");
    assert_eq!(line.end(), LineEnd::Delim);
}

#[test]
fn test_line_multi_char_delimiter_set() {
    let mut line = Line::new(Some(b":|"), false, None);
    for &b in b"ab|cd" {
        line.feed(b);
    }
    assert!(line.finished());
    assert_eq!(line.buf, b"ab");
    assert_eq!(line.end(), LineEnd::Delim);
}

#[test]
fn test_line_empty_delim_reads_to_eof() {
    let mut line = Line::new(Some(b""), false, None);
    for &b in b"a:b:c" {
        line.feed(b);
    }
    assert!(!line.finished());
    line.end_eof();
    assert_eq!(line.buf, b"a:b:c");
    // bash: EOF is still a failure even when the delimiter set is empty.
    assert_eq!(line.end(), LineEnd::Eof);
}

#[test]
fn test_line_escaped_delimiter_kept() {
    let mut line = Line::new(Some(b":"), false, None);
    for &b in b"a\\:b:c" {
        line.feed(b);
    }
    assert!(line.finished());
    assert_eq!(line.buf, b"a:b");
}

#[test]
fn test_line_max_cap_post_processing() {
    let mut line = Line::new(None, false, Some(2));
    for &b in b"a\\bc" {
        line.feed(b);
    }
    assert!(line.finished());
    assert_eq!(line.buf, b"ab");
    assert_eq!(line.end(), LineEnd::Delim);
}

#[test]
fn test_line_n_zero_done_before_any_feed() {
    let mut line = Line::new(None, false, Some(0));
    line.feed(b'a');
    assert!(line.finished());
    assert!(line.buf.is_empty());
    assert_eq!(line.end(), LineEnd::Delim);
}

#[test]
fn test_line_feed_noop_after_finished() {
    let mut line = Line::new(None, false, Some(1));
    for &b in b"ab" {
        line.feed(b);
    }
    assert_eq!(line.buf, b"a");
}

// read_from_fd tests

#[test]
fn test_read_line_from_fd_eof() {
    let (read_end, write_end) = sys::pipe::pipe2(0).unwrap();
    // Close write end immediately → EOF
    drop(write_end);

    let mut line = Line::new(None, false, None);
    read_line_from_fd(|b: &mut [u8]| read_end.read(b), &mut line).unwrap();
    assert_eq!(line.end(), LineEnd::Eof);
    assert!(line.buf.is_empty());
}

#[test]
fn test_read_line_from_fd_max_bytes() {
    let (read_end, write_end) = sys::pipe::pipe2(0).unwrap();
    let data = b"hello world";
    write_end.write(data).unwrap();
    drop(write_end);

    let mut line = Line::new(None, false, Some(5));
    read_line_from_fd(|b: &mut [u8]| read_end.read(b), &mut line).unwrap();
    assert_eq!(line.buf, b"hello");
    assert_eq!(line.end(), LineEnd::Delim);
}

#[test]
fn test_read_line_from_fd_stops_at_newline() {
    let (read_end, write_end) = sys::pipe::pipe2(0).unwrap();
    let data = b"line1\nline2";
    write_end.write(data).unwrap();
    drop(write_end);

    let mut line = Line::new(None, false, None);
    read_line_from_fd(|b: &mut [u8]| read_end.read(b), &mut line).unwrap();
    assert_eq!(line.buf, b"line1");
    assert_eq!(line.end(), LineEnd::Delim);
}

#[test]
fn test_read_line_from_fd_empty_delim_whole_stream() {
    let (read_end, write_end) = sys::pipe::pipe2(0).unwrap();
    let data = b"a:b:c";
    write_end.write(data).unwrap();
    drop(write_end);

    let mut line = Line::new(Some(b""), false, None);
    read_line_from_fd(|b: &mut [u8]| read_end.read(b), &mut line).unwrap();
    assert_eq!(line.buf, b"a:b:c");
    assert_eq!(line.end(), LineEnd::Eof);
}

#[test]
fn test_read_line_from_fd_colon_delimiter() {
    let (read_end, write_end) = sys::pipe::pipe2(0).unwrap();
    let data = b"a:b:c\n";
    write_end.write(data).unwrap();
    drop(write_end);

    let mut line = Line::new(Some(b":"), false, None);
    read_line_from_fd(|b: &mut [u8]| read_end.read(b), &mut line).unwrap();
    assert_eq!(line.buf, b"a");
    assert_eq!(line.end(), LineEnd::Delim);
}

#[test]
fn test_read_line_from_fd_backslash_processed() {
    let (read_end, write_end) = sys::pipe::pipe2(0).unwrap();
    let data = b"a\\nb\n";
    write_end.write(data).unwrap();
    drop(write_end);

    let mut line = Line::new(None, false, None);
    read_line_from_fd(|b: &mut [u8]| read_end.read(b), &mut line).unwrap();
    assert_eq!(line.buf, b"anb");
}

#[test]
fn test_read_line_from_fd_raw_keeps_backslash() {
    let (read_end, write_end) = sys::pipe::pipe2(0).unwrap();
    let data = b"a\\nb\n";
    write_end.write(data).unwrap();
    drop(write_end);

    let mut line = Line::new(None, true, None);
    read_line_from_fd(|b: &mut [u8]| read_end.read(b), &mut line).unwrap();
    assert_eq!(line.buf, b"a\\nb");
}

#[test]
fn test_read_line_from_fd_error() {
    let mut line = Line::new(None, false, None);
    let result = read_line_from_fd(|_b: &mut [u8]| Err(SyscallError::EBADF("read")), &mut line);
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err().current_context(),
        CmdError::Read
    ));
    assert!(!line.finished());
    assert!(line.buf.is_empty());
}

#[test]
fn test_read_line_from_fd_multi_chunk() {
    // The writer runs in a forked child: pre-writing >4 KiB into a pipe
    // deadlocks when the kernel has shrunk the pipe (system pipe-page
    // pressure), because the write blocks before any reader exists.
    let (read_end, write_end) = sys::pipe::pipe2(0).unwrap();
    let data = [b'x'; 8192];
    let (_, pidfd_opt) = sys::fork_pidfd::fork_pidfd().unwrap();
    match pidfd_opt {
        None => {
            // Child: the write may block until the parent drains the pipe.
            write_end.write(&data).unwrap();
            write_end.write(b"\n").unwrap();
            drop(write_end);
            sys::exit(0);
        }
        Some(pidfd) => {
            let mut line = Line::new(None, false, None);
            read_line_from_fd(|b: &mut [u8]| read_end.read(b), &mut line).unwrap();
            assert_eq!(line.end(), LineEnd::Delim);
            assert_eq!(line.buf.len(), 8192);
            assert!(line.buf.iter().all(|b| *b == b'x'));
            drop(read_end);
            match pidfd.wait_pidfd().unwrap() {
                WaitStatus::Exited(0) => {}
                other => panic!("unexpected status {}", other.exit_code()),
            }
        }
    }
}

// read_line tests via SourceFd::RawFd

fn rawfd_source(read_end: &sys::LocalFd) -> SourceFd {
    let exported = read_end.export().unwrap();
    SourceFd::RawFd(
        sys::ShortCStr::from_vec(format!("{}", exported.as_raw()).into_bytes()).unwrap(),
    )
}

#[test]
fn test_read_line_rawfd_eof() {
    let (read_end, write_end) = sys::pipe::pipe2(0).unwrap();
    drop(write_end);

    let source = rawfd_source(&read_end);
    let result = read_line(&source, None, &make_flags(None, false, None, None));
    assert!(result.is_ok());
    let (buf, end) = result.unwrap();
    assert_eq!(end, LineEnd::Eof);
    assert!(buf.is_empty());
}

#[test]
fn test_read_line_rawfd_data() {
    let (read_end, write_end) = sys::pipe::pipe2(0).unwrap();
    let data = b"hello world\n";
    write_end.write(data).unwrap();
    drop(write_end);

    let source = rawfd_source(&read_end);
    let result = read_line(&source, None, &make_flags(None, false, None, None));
    assert!(result.is_ok());
    let (buf, end) = result.unwrap();
    assert_eq!(end, LineEnd::Delim);
    assert_eq!(buf, b"hello world");
}

#[test]
fn test_read_line_rawfd_max_bytes() {
    let (read_end, write_end) = sys::pipe::pipe2(0).unwrap();
    let data = b"hello world\n";
    write_end.write(data).unwrap();
    drop(write_end);

    let source = rawfd_source(&read_end);
    let result = read_line(&source, None, &make_flags(None, false, Some(5), None));
    assert!(result.is_ok());
    let (buf, end) = result.unwrap();
    assert_eq!(end, LineEnd::Delim);
    assert_eq!(buf, b"hello");
}

#[test]
fn test_read_line_rawfd_stops_at_newline() {
    let (read_end, write_end) = sys::pipe::pipe2(0).unwrap();
    let data = b"first\nsecond\n";
    write_end.write(data).unwrap();
    drop(write_end);

    let source = rawfd_source(&read_end);
    let result = read_line(&source, None, &make_flags(None, false, None, None));
    assert!(result.is_ok());
    let (buf, end) = result.unwrap();
    assert_eq!(end, LineEnd::Delim);
    assert_eq!(buf, b"first");
}

#[test]
fn test_read_line_rawfd_empty_delim() {
    let (read_end, write_end) = sys::pipe::pipe2(0).unwrap();
    let data = b"a:b:c";
    write_end.write(data).unwrap();
    drop(write_end);

    let source = rawfd_source(&read_end);
    let result = read_line(&source, None, &make_flags(Some(b""), false, None, None));
    assert!(result.is_ok());
    let (buf, end) = result.unwrap();
    assert_eq!(end, LineEnd::Eof);
    assert_eq!(buf, b"a:b:c");
}

#[test]
fn test_read_line_rawfd_colon_delimiter() {
    let (read_end, write_end) = sys::pipe::pipe2(0).unwrap();
    let data = b"a:b:c\n";
    write_end.write(data).unwrap();
    drop(write_end);

    let source = rawfd_source(&read_end);
    let result = read_line(&source, None, &make_flags(Some(b":"), false, None, None));
    assert!(result.is_ok());
    let (buf, end) = result.unwrap();
    assert_eq!(end, LineEnd::Delim);
    assert_eq!(buf, b"a");
}

#[test]
fn test_read_line_rawfd_n_zero() {
    let (read_end, write_end) = sys::pipe::pipe2(0).unwrap();
    let data = b"hello\n";
    write_end.write(data).unwrap();
    drop(write_end);

    let source = rawfd_source(&read_end);
    let result = read_line(&source, None, &make_flags(None, false, Some(0), None));
    assert!(result.is_ok());
    let (buf, end) = result.unwrap();
    assert_eq!(end, LineEnd::Delim);
    assert!(buf.is_empty());
}

#[test]
fn test_read_line_rawfd_timeout_zero_data_ready() {
    let (read_end, write_end) = sys::pipe::pipe2(0).unwrap();
    let data = b"hi\n";
    write_end.write(data).unwrap();
    drop(write_end);

    let source = rawfd_source(&read_end);
    let result = read_line(&source, None, &make_flags(None, false, None, Some(0)));
    assert!(result.is_ok());
    let (buf, end) = result.unwrap();
    assert_eq!(end, LineEnd::Delim);
    assert_eq!(buf, b"hi");
}

#[test]
fn test_read_line_rawfd_timeout_zero_no_data() {
    let (read_end, write_end) = sys::pipe::pipe2(0).unwrap();
    // Keep the write end open: an empty pipe with no EOF is not ready.
    let source = rawfd_source(&read_end);
    let result = read_line(&source, None, &make_flags(None, false, None, Some(0)));
    assert!(result.is_ok());
    let (buf, end) = result.unwrap();
    assert_eq!(end, LineEnd::Timeout);
    assert!(buf.is_empty());
    drop(write_end);
}

#[test]
fn test_read_line_fdvar_no_clone() {
    let source = SourceFd::FdVar(c"MYVAR".into());
    let result = read_line(&source, None, &make_flags(None, false, None, None));
    assert!(result.is_ok());
    let (buf, end) = result.unwrap();
    assert_eq!(end, LineEnd::Eof);
    assert!(buf.is_empty());
}

#[test]
fn test_read_line_fdvar_with_clone() {
    let (read_end, write_end) = sys::pipe::pipe2(0).unwrap();
    let data = b"from var\n";
    write_end.write(data).unwrap();
    drop(write_end);

    let source = SourceFd::FdVar(c"MYVAR".into());
    let result = read_line(
        &source,
        Some(&read_end),
        &make_flags(None, false, None, None),
    );
    assert!(result.is_ok());
    let (buf, end) = result.unwrap();
    assert_eq!(end, LineEnd::Delim);
    assert_eq!(buf, b"from var");
}

// read_line tests via SourceFd::Stdin (fork + dup2 onto fd 0)

fn stdin_read_line(data: &[u8], flags: &ReadFlags) -> (Vec<u8>, LineEnd) {
    let (res_r, res_w) = sys::pipe::pipe2(0).unwrap();
    let (data_r, data_w) = sys::pipe::pipe2(0).unwrap();
    if !data.is_empty() {
        data_w.write(data).unwrap();
    }
    drop(data_w);

    match sys::fork_pidfd::fork_pidfd().unwrap().1 {
        None => {
            data_r.export_to(0).unwrap();
            drop(data_r);
            let (buf, end) = match read_line(&SourceFd::Stdin, None, flags) {
                Ok(v) => v,
                Err(_) => sys::exit(3),
            };
            res_w.write(&buf).unwrap();
            let code = match end {
                LineEnd::Delim => 0u8,
                LineEnd::Eof => 1,
                LineEnd::Timeout => 2,
            };
            res_w.write(&[code]).unwrap();
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
            match pidfd.wait_pidfd().unwrap() {
                WaitStatus::Exited(0) => {}
                other => panic!("child failed: {}", other.exit_code()),
            }
            let end = match out.pop().unwrap_or(0) {
                0 => LineEnd::Delim,
                1 => LineEnd::Eof,
                _ => LineEnd::Timeout,
            };
            (out, end)
        }
    }
}

#[test]
fn test_read_line_stdin_data() {
    let (buf, end) = stdin_read_line(b"abc\n", &make_flags(None, false, None, None));
    assert_eq!(end, LineEnd::Delim);
    assert_eq!(buf, b"abc");
}

#[test]
fn test_read_line_stdin_eof() {
    let (buf, end) = stdin_read_line(b"", &make_flags(None, false, None, None));
    assert_eq!(end, LineEnd::Eof);
    assert!(buf.is_empty());
}

#[test]
fn test_read_line_stdin_max_bytes() {
    let (buf, end) = stdin_read_line(b"hello world\n", &make_flags(None, false, Some(5), None));
    assert_eq!(end, LineEnd::Delim);
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
    write_end.write(data).unwrap();
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
    cmdline.prefix = BuiltinPrefix::Builtin;
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
        cap: None,
        set_at: sys::Position::new(1, 1),
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
    write_end.write(data).unwrap();
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
    write_end.write(b"hello").unwrap();
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
    // bash: EOF before the delimiter → status 1, even with partial data.
    assert!(matches!(state.last_status, WaitStatus::Exited(1)));
}

#[test]
fn run_read_writes_prompt_to_stderr() {
    let (data_r, data_w) = sys::pipe::pipe2(0).unwrap();
    data_w.write(b"answer\n").unwrap();
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
            match pidfd.wait_pidfd().unwrap() {
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
    write_end.write(data).unwrap();
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
    write_end.write(data).unwrap();
    drop(write_end);

    let cell = make_read_cell();
    {
        let mut state = cell.borrow_mut().unwrap();
        state.fds.insert(
            c"MYFD".into(),
            crate::state::FdVar {
                fd: read_end,
                trace: sys::Trace::boundary(sys::Origin::Shell),
            },
        );
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
    write_end.write(data).unwrap();
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
    write_end.write(data).unwrap();
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
    write_end.write(data).unwrap();
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
    write_end.write(data).unwrap();
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
    write_end.write(data).unwrap();
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
    write_end.write(data).unwrap();
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
    write_end.write(data).unwrap();
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

#[test]
fn run_read_default_backslash_processing() {
    let (read_end, write_end) = sys::pipe::pipe2(0).unwrap();
    write_end.write(b"a\\nb\n").unwrap();
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
        Some(&c"anb".into())
    );
    assert!(matches!(state.last_status, WaitStatus::Exited(0)));
}

#[test]
fn run_read_r_keeps_backslash_literal() {
    let (read_end, write_end) = sys::pipe::pipe2(0).unwrap();
    write_end.write(b"a\\nb\n").unwrap();
    drop(write_end);

    let exported = read_end.export().unwrap();
    let fd = exported.as_raw();
    let args = vec![c"-r".into(), c"var1".into()];
    let line = make_read_u_line(&args, fd);
    let cmdline = make_read_u_cmdline(&args, fd);
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
        Some(&c"a\\nb".into())
    );
}

#[test]
fn run_read_empty_delim_reads_to_eof() {
    let (read_end, write_end) = sys::pipe::pipe2(0).unwrap();
    write_end.write(b"a:b:c").unwrap();
    drop(write_end);

    let exported = read_end.export().unwrap();
    let fd = exported.as_raw();
    let args = vec![c"-d".into(), c"".into(), c"var1".into()];
    let line = make_read_u_line(&args, fd);
    let cmdline = make_read_u_cmdline(&args, fd);
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
        Some(&c"a:b:c".into())
    );
    // bash: EOF is a failure even for `-d ''` → status 1, data kept.
    assert!(matches!(state.last_status, WaitStatus::Exited(1)));
}

#[test]
fn run_read_colon_delimiter() {
    let (read_end, write_end) = sys::pipe::pipe2(0).unwrap();
    write_end.write(b"a:b:c\n").unwrap();
    drop(write_end);

    let exported = read_end.export().unwrap();
    let fd = exported.as_raw();
    let args = vec![c"-d".into(), c":".into(), c"var1".into()];
    let line = make_read_u_line(&args, fd);
    let cmdline = make_read_u_cmdline(&args, fd);
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
        Some(&c"a".into())
    );
    assert!(matches!(state.last_status, WaitStatus::Exited(0)));
}

#[test]
fn run_read_timeout_zero_no_data_status_1_unset() {
    let (read_end, write_end) = sys::pipe::pipe2(0).unwrap();
    // Keep the write end open: no data, no EOF.
    let exported = read_end.export().unwrap();
    let fd = exported.as_raw();
    let args = vec![c"-t".into(), c"0".into(), c"var1".into()];
    let line = make_read_u_line(&args, fd);
    let cmdline = make_read_u_cmdline(&args, fd);
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
    drop(write_end);
}

#[test]
fn run_read_timeout_zero_data_ready_status_0() {
    let (read_end, write_end) = sys::pipe::pipe2(0).unwrap();
    write_end.write(b"hi\n").unwrap();
    drop(write_end);

    let exported = read_end.export().unwrap();
    let fd = exported.as_raw();
    let args = vec![c"-t".into(), c"0".into(), c"var1".into()];
    let line = make_read_u_line(&args, fd);
    let cmdline = make_read_u_cmdline(&args, fd);
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
        Some(&c"hi".into())
    );
    assert!(matches!(state.last_status, WaitStatus::Exited(0)));
}

#[test]
fn run_read_n_zero_empty_vars_status_0() {
    let (read_end, write_end) = sys::pipe::pipe2(0).unwrap();
    write_end.write(b"hello\n").unwrap();
    drop(write_end);

    let exported = read_end.export().unwrap();
    let fd = exported.as_raw();
    let args = vec![c"-n".into(), c"0".into(), c"var1".into()];
    let line = make_read_u_line(&args, fd);
    let cmdline = make_read_u_cmdline(&args, fd);
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
        Some(&ShortCStr::new())
    );
    assert!(matches!(state.last_status, WaitStatus::Exited(0)));
}
