use error_stack::{Report, ResultExt};

use crate::{ImportedFd, ShortCStr, SyscallError};

impl ImportedFd {
    /// Read bytes from the fd into buf.
    pub fn read(&self, buf: &mut [u8]) -> Result<usize, SyscallError> {
        // SAFETY: `buf` is a valid mutable slice; `read` won't write past `buf.len()`.
        crate::cvt(unsafe { libc::read(self.as_raw(), buf.as_mut_ptr().cast(), buf.len()) })
            .map(|n| n as usize)
    }

    /// Write bytes to the fd.
    pub fn write(&self, buf: &[u8]) -> Result<usize, SyscallError> {
        // SAFETY: `buf` is a valid slice; `write` won't write past `buf.len()`.
        crate::cvt(unsafe { libc::write(self.as_raw(), buf.as_ptr().cast(), buf.len()) })
            .map(|n| n as usize)
    }

    /// Write all bytes, retrying on short writes.
    pub fn write_all(&self, buf: &[u8]) -> Result<(), SyscallError> {
        let mut written = 0;
        while written < buf.len() {
            let slice = buf.get(written..).ok_or(SyscallError::Never)?;
            let n = self.write(slice)?;
            if n == 0 {
                break;
            }
            written += n;
        }
        Ok(())
    }

    /// Write a ShortCStr to the fd.
    pub fn write_str(&self, s: &ShortCStr) -> Result<(), Report<SyscallError>> {
        Ok(self.write_all(s.as_bytes().change_context(SyscallError::Never)?)?)
    }

    /// Read until EOF or buffer full.
    pub fn read_all(&self, buf: &mut [u8]) -> Result<usize, SyscallError> {
        let mut offset = 0;
        loop {
            let slice = buf.get_mut(offset..).ok_or(SyscallError::Never)?;
            match self.read(slice)? {
                0 => break,
                n => offset += n,
            }
            if offset >= buf.len() {
                break;
            }
        }
        Ok(offset)
    }
}
