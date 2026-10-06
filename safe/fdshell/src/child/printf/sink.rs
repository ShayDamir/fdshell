//! The render targets: stdout bytes, stderr diagnostics, and the failure flag.

use alloc::vec::Vec;

/// The render targets: the standard-output bytes, the standard-error
/// diagnostics, and whether any numeric failure occurred.
pub struct Sink {
    pub out: Vec<u8>,
    pub err: Vec<u8>,
    pub failed: bool,
}

impl Sink {
    pub fn new() -> Self {
        Self {
            out: Vec::new(),
            err: Vec::new(),
            failed: false,
        }
    }

    /// Record a numeric-argument failure: `printf: <arg>: <reason>` on stderr.
    pub fn report_num(&mut self, arg: &[u8], reason: &str) {
        self.failed = true;
        self.err.extend_from_slice(b"printf: ");
        self.err.extend_from_slice(arg);
        self.err.extend_from_slice(b": ");
        self.err.extend_from_slice(reason.as_bytes());
        self.err.push(b'\n');
    }
}
