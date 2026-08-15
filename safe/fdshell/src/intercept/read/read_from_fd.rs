use crate::error::cmd::CmdError;
use alloc::vec::Vec;
use error_stack::{Report, ResultExt};
use sys::SyscallError;

pub(crate) fn read_line_from_fd(
    mut read: impl FnMut(&mut [u8]) -> Result<usize, SyscallError>,
    buf: &mut Vec<u8>,
    eof: &mut bool,
    max_bytes: Option<usize>,
) -> Result<(), Report<CmdError>> {
    let mut temp = [0u8; 4096];
    loop {
        let mut done = false;
        let n = read(&mut temp).change_context(CmdError::Read)?;
        if n == 0 {
            *eof = true;
            break;
        }
        for &b in temp.get(..n).ok_or(CmdError::Never)? {
            if b == b'\n' {
                done = true;
                break;
            }
            buf.push(b);
            if let Some(max) = max_bytes
                && buf.len() >= max
            {
                done = true;
                break;
            }
        }
        if *eof || done {
            break;
        }
    }
    Ok(())
}
