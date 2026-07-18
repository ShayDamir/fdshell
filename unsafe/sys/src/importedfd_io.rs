//! I/O operations for ImportedFd — read, write, read_all, write_all.

/// Extension trait providing I/O operations on ImportedFd.
pub trait ImportedFdIo {
    /// Read bytes from the fd into buf.
    fn read(&self, buf: &mut [u8]) -> Result<isize, crate::SyscallError>;

    /// Write bytes to the fd.
    fn write(&self, buf: &[u8]) -> Result<isize, crate::SyscallError>;

    /// Write all bytes, retrying on short writes.
    fn write_all(&self, buf: &[u8]) -> Result<(), crate::SyscallError>;

    /// Read until EOF or buffer full.
    fn read_all(&self, buf: &mut [u8]) -> Result<usize, crate::SyscallError>;
}

impl ImportedFdIo for crate::ImportedFd {
    fn read(&self, buf: &mut [u8]) -> Result<isize, crate::SyscallError> {
        // SAFETY: `buf` is a valid mutable slice; `read` won't write past `buf.len()`.
        crate::cvt(unsafe { libc::read(self.as_raw(), buf.as_mut_ptr().cast(), buf.len()) })
    }

    /// Write bytes to the fd.
    fn write(&self, buf: &[u8]) -> Result<isize, crate::SyscallError> {
        // SAFETY: `buf` is a valid slice; `write` won't write past `buf.len()`.
        crate::cvt(unsafe { libc::write(self.as_raw(), buf.as_ptr().cast(), buf.len()) })
    }

    /// Write all bytes, retrying on short writes.
    fn write_all(&self, buf: &[u8]) -> Result<(), crate::SyscallError> {
        let mut written = 0;
        while written < buf.len() {
            let slice = buf.get(written..).ok_or(crate::SyscallError::Never)?;
            let n = self.write(slice)?;
            if n == 0 {
                break;
            }
            written += n as usize;
        }
        Ok(())
    }

    /// Read until EOF or buffer full.
    fn read_all(&self, buf: &mut [u8]) -> Result<usize, crate::SyscallError> {
        let mut offset = 0;
        loop {
            let slice = buf.get_mut(offset..).ok_or(crate::SyscallError::Never)?;
            match self.read(slice)? {
                0 => break,
                n => offset += n as usize,
            }
            if offset >= buf.len() {
                break;
            }
        }
        Ok(offset)
    }
}
