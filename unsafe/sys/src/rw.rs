use crate::LocalFd;

pub fn read(fd: &LocalFd, buf: &mut [u8]) -> Result<usize, crate::SyscallError> {
    // SAFETY: `buf` is a valid mutable slice; `read` won't write past `buf.len()`.
    crate::cvt(unsafe { libc::read(fd.as_raw(), buf.as_mut_ptr().cast(), buf.len()) })
        .map(|n| n as usize)
}

pub fn write(fd: &LocalFd, buf: &[u8]) -> Result<usize, crate::SyscallError> {
    // SAFETY: `buf` is a valid immutable slice; `write` won't read past `buf.len()`.
    crate::cvt(unsafe { libc::write(fd.as_raw(), buf.as_ptr().cast(), buf.len()) })
        .map(|n| n as usize)
}

/// Read until EOF or buffer full.
pub fn read_all(fd: &LocalFd, buf: &mut [u8]) -> Result<usize, crate::SyscallError> {
    let mut offset = 0;
    loop {
        let slice = buf
            .get_mut(offset..)
            .ok_or(crate::SyscallError::EINVAL("buffer full"))?;
        match read(fd, slice)? {
            0 => break,
            n => offset += n,
        }
        if offset >= buf.len() {
            break;
        }
    }
    Ok(offset)
}
