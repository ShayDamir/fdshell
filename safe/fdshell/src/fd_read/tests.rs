use super::*;
use crate::error::cmd::CmdError;
use alloc::collections::VecDeque;
use alloc::vec;
use alloc::vec::Vec;
use error_stack::Report;
use sys::SyscallError;

/// The read the loop drives: `answers` are the syscall's answers in order (a
/// byte count hands over that many bytes of `data`; running out answers EOF),
/// and `calls` counts the reads so a row can show the loop stopped before the
/// next one.
struct Scripted {
    answers: VecDeque<Result<usize, SyscallError>>,
    data: Vec<u8>,
    consumed: usize,
    calls: usize,
}

impl Scripted {
    fn new(data: &[u8], answers: Vec<Result<usize, SyscallError>>) -> Self {
        Self {
            answers: VecDeque::from(answers),
            data: data.to_vec(),
            consumed: 0,
            calls: 0,
        }
    }

    fn read_one(&mut self, buf: &mut [u8]) -> Result<usize, SyscallError> {
        self.calls += 1;
        let Some(answer) = self.answers.pop_front() else {
            return Ok(0);
        };
        let Ok(n) = answer else {
            return answer;
        };
        if n > buf.len() {
            return Ok(n);
        }
        let take = &self.data[self.consumed..self.consumed + n];
        buf[..n].copy_from_slice(take);
        self.consumed += n;
        Ok(n)
    }

    /// The bytes the first `Ok(..)` answer asked for.
    fn first(&self, n: usize) -> Vec<u8> {
        self.data[..n].to_vec()
    }
}

/// The loop under a sink that collects every chunk and asks the row's own
/// predicate what to do with it; `collected` is what the sink saw.
fn drive(
    script: &mut Scripted,
    collected: &mut Vec<u8>,
    sink: &dyn Fn(&[u8]) -> Decision<CmdError>,
    on_error: &dyn Fn(&SyscallError) -> Decision<CmdError>,
    on_never: &dyn Fn() -> Decision<CmdError>,
) -> Result<(), Report<CmdError>> {
    read_to_end(
        &mut |buf| script.read_one(buf),
        &mut |chunk| {
            collected.extend_from_slice(chunk);
            sink(chunk)
        },
        on_error,
        on_never,
    )
}

/// Every row's default: keep reading, nothing to report, `Never` for the
/// slice a read cannot produce.
fn keep_reading(_chunk: &[u8]) -> Decision<CmdError> {
    Decision::Resume
}

fn never_report(_err: &SyscallError) -> Decision<CmdError> {
    Decision::Report(Report::new(CmdError::Never))
}

fn never_never() -> Decision<CmdError> {
    Decision::Report(Report::new(CmdError::Never))
}

#[test]
fn every_chunk_reaches_the_sink_until_eof() {
    let mut script = Scripted::new(b"abcdefg", vec![Ok(3), Ok(4)]);
    let mut collected = Vec::new();
    drive(
        &mut script,
        &mut collected,
        &keep_reading,
        &never_report,
        &never_never,
    )
    .unwrap();
    assert_eq!(collected, b"abcdefg".to_vec());
    assert_eq!(script.calls, 3, "two chunks, then the EOF that ends it");
}

#[test]
fn eof_after_a_partial_chunk_keeps_what_was_read() {
    let mut script = Scripted::new(b"abcde", vec![Ok(3), Ok(2)]);
    let mut collected = Vec::new();
    drive(
        &mut script,
        &mut collected,
        &keep_reading,
        &never_report,
        &never_never,
    )
    .unwrap();
    assert_eq!(collected, b"abcde".to_vec());
    assert_eq!(script.calls, 3);
}

#[test]
fn the_sinks_stop_ends_the_read_before_the_next_one() {
    let mut script = Scripted::new(b"abcdefg", vec![Ok(3), Ok(4)]);
    let mut collected = Vec::new();
    let stop = |_chunk: &[u8]| Decision::Done;
    drive(
        &mut script,
        &mut collected,
        &stop,
        &never_report,
        &never_never,
    )
    .unwrap();
    assert_eq!(collected, script.first(3), "the chunk is handed over first");
    assert_eq!(script.calls, 1, "the stop is honoured before the next read");
}

#[test]
fn the_sinks_report_propagates_the_cap_decision() {
    let mut script = Scripted::new(b"abcdefg", vec![Ok(3)]);
    let mut collected = Vec::new();
    let over = |_chunk: &[u8]| Decision::Report(Report::new(CmdError::SourceTooLarge));
    let e = drive(
        &mut script,
        &mut collected,
        &over,
        &never_report,
        &never_never,
    )
    .unwrap_err();
    assert!(matches!(e.current_context(), CmdError::SourceTooLarge));
    assert_eq!(script.calls, 1);
}

#[test]
fn resume_on_error_reads_again_and_completes() {
    let mut script = Scripted::new(
        b"abcdefg",
        vec![
            Err(SyscallError::Other {
                errno: sys::errno::EINTR,
                syscall: "read",
            }),
            Ok(7),
        ],
    );
    let mut collected = Vec::new();
    let resume = |err: &SyscallError| {
        assert_eq!(
            err.errno(),
            sys::errno::EINTR,
            "the interrupt is the reason"
        );
        Decision::Resume
    };
    drive(
        &mut script,
        &mut collected,
        &keep_reading,
        &resume,
        &never_never,
    )
    .unwrap();
    assert_eq!(collected, b"abcdefg".to_vec(), "the read carried on");
    assert_eq!(script.calls, 3);
}

#[test]
fn report_on_error_propagates_the_sites_own_chain() {
    let mut script = Scripted::new(b"abc", vec![Err(SyscallError::EBADF("read"))]);
    let mut collected = Vec::new();
    let report =
        |err: &SyscallError| Decision::Report(Report::new(*err).change_context(CmdError::Read));
    let e = drive(
        &mut script,
        &mut collected,
        &keep_reading,
        &report,
        &never_never,
    )
    .unwrap_err();
    assert!(matches!(e.current_context(), CmdError::Read));
    assert_eq!(collected.len(), 0, "nothing was read to keep");
    assert_eq!(script.calls, 1);
}

#[test]
fn done_on_error_keeps_the_bytes_read_so_far() {
    let mut script = Scripted::new(b"abcdefg", vec![Ok(3), Err(SyscallError::EBADF("read"))]);
    let mut collected = Vec::new();
    let stop = |_err: &SyscallError| Decision::Done;
    drive(
        &mut script,
        &mut collected,
        &keep_reading,
        &stop,
        &never_never,
    )
    .unwrap();
    assert_eq!(
        collected,
        script.first(3),
        "the bytes so far are the answer"
    );
    assert_eq!(script.calls, 2);
}

#[test]
fn the_impossible_slice_answers_the_sites_own_never() {
    let mut script = Scripted::new(b"abc", vec![Ok(CHUNK + 1)]);
    let mut collected = Vec::new();
    let e = drive(
        &mut script,
        &mut collected,
        &keep_reading,
        &never_report,
        &never_never,
    )
    .unwrap_err();
    assert!(matches!(e.current_context(), CmdError::Never));
    assert_eq!(
        collected.len(),
        0,
        "the sink never saw the impossible chunk"
    );
}
