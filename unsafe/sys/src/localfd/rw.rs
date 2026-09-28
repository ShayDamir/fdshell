use crate::{LocalFd, SyscallError};

impl LocalFd {
    pub fn write(&self, buf: &[u8]) -> Result<usize, SyscallError> {
        // SAFETY: `buf` is a valid immutable slice; `write` won't read past `buf.len()`.
        crate::cvt(unsafe { libc::write(self.as_raw(), buf.as_ptr().cast(), buf.len()) })
            .map(|n| n as usize)
    }

    /// Write all of `buf`, retrying on partial writes.
    pub fn write_all(&self, buf: &[u8]) -> Result<(), SyscallError> {
        let mut off = 0usize;
        while off < buf.len() {
            let slice = buf.get(off..).ok_or(SyscallError::Never)?;
            let n = self.write(slice)?;
            off = off.checked_add(n).ok_or(SyscallError::Never)?;
        }
        Ok(())
    }

    /// Move the file offset; return the new absolute position.
    pub fn lseek(&self, offset: i64, whence: i32) -> Result<i64, SyscallError> {
        // SAFETY: `self` is a valid open fd; `lseek` returns the new offset or -1 on error.
        crate::cvt64(unsafe { libc::lseek(self.as_raw(), offset, whence) })
    }
}
