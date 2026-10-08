/// Parsed `read` flags.
#[derive(Debug)]
pub(crate) struct ReadFlags<'a> {
    pub source: SourceFd,
    pub max_bytes: Option<usize>,
    pub prompt: Option<&'a [u8]>,
    pub raw: bool,
    pub delim: Option<&'a [u8]>,
    pub timeout: Option<u32>,
}

impl<'a> ReadFlags<'a> {
    /// The `-t` value in milliseconds, saturating to `-1` (infinite) when
    /// the product would overflow `i32`.
    pub fn timeout_ms(&self) -> Option<i32> {
        let secs = self.timeout?;
        if secs > i32::MAX as u32 / 1000 {
            return Some(-1);
        }
        Some(secs as i32 * 1000)
    }
}

pub(crate) use parse::parse_flags;
pub(crate) use source::SourceFd;

mod parse;
mod source;
