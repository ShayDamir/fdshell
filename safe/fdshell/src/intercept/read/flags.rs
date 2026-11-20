// mutants: -C --lib -C --test -C read -C --test -C trap -- -E 'test(/^intercept::read::tests::/) | binary(read) | binary(trap)'

use crate::error::cmd::CmdError;
use error_stack::{Report, ResultExt};
use sys::ImportedFd;

/// Parsed `read` flags.
#[cfg_attr(test, derive(Debug))]
pub(crate) struct ReadFlags<'a> {
    pub source: SourceFd,
    pub max_bytes: Option<usize>,
    /// The `-p` operand's **word index** in the caller's argument list, not its
    /// bytes: the operand is an ordinary word, so the call site expands it with
    /// its own mask, spans and quoted bit, which are keyed by word index.
    pub prompt_word: Option<usize>,
    pub raw: bool,
    pub delim: Option<&'a [u8]>,
    pub timeout: Option<u64>,
}

impl<'a> ReadFlags<'a> {
    /// The `-t` value in whole milliseconds (the grammar answers milliseconds),
    /// saturating to `-1` (infinite) when it exceeds `i32`. The `u64` rows that
    /// bash answers non-monotonically (measured: `4294967296` returns
    /// immediately) are not the contract; the saturation is (README).
    pub fn timeout_ms(&self) -> Option<i32> {
        let ms = self.timeout?;
        if ms > i32::MAX as u64 {
            return Some(-1);
        }
        Some(ms as i32)
    }
}

pub(crate) use parse::parse_flags;
pub(crate) use source::SourceFd;

/// True when the fd `read` reads from — stdin, `-u <rawfd>`, or a resolved fd
/// var — is a terminal: bash writes the `-p` prompt only when it is, and the fd
/// it tests is the resolved source, not necessarily stdin.
///
/// A clone-less fd var has no fd to test, so it answers *not a terminal*: bash's
/// rule is that the tested fd is the fd read from, and with no fd nothing passes
/// the test — the same visible answer as the non-tty branch, and consistent with
/// `io::read_line`, which reads nothing from a clone-less fd var.
pub(crate) fn source_is_tty(
    source: &SourceFd,
    fd_clone: Option<&sys::LocalFd>,
) -> Result<bool, Report<CmdError>> {
    match source {
        SourceFd::Stdin => sys::IN.is_tty().change_context(CmdError::Read),
        SourceFd::RawFd(num) => {
            let fd = ImportedFd::try_from(num).change_context(CmdError::Read)?;
            fd.is_tty().change_context(CmdError::Read)
        }
        SourceFd::FdVar(_) => Ok(fd_clone.is_some_and(|fd| fd.is_tty())),
    }
}

mod parse;
mod source;
