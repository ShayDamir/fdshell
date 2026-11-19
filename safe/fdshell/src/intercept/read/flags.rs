// mutants: -C --lib -C --test -C read -C --test -C trap -- -E 'test(/^intercept::read::tests::/) | binary(read) | binary(trap)'

/// Parsed `read` flags.
#[cfg_attr(test, derive(Debug))]
pub(crate) struct ReadFlags<'a> {
    pub source: SourceFd,
    pub max_bytes: Option<usize>,
    pub prompt: Option<&'a [u8]>,
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

mod parse;
mod source;
