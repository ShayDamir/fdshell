use crate::error::cmd::CmdError;
use alloc::vec::Vec;
use error_stack::{Report, ResultExt};

use super::flags::SourceFd;
use super::read_from_fd::read_line_from_fd;

pub(crate) fn read_line(
    source: &SourceFd,
    fd_clone: Option<&sys::LocalFd>,
    max_bytes: Option<usize>,
) -> Result<(Vec<u8>, bool), Report<CmdError>> {
    let mut buf = Vec::new();
    let mut eof = false;

    match source {
        SourceFd::Stdin => {
            let mut byte = [0u8; 1];
            loop {
                let n = sys::IN.read(&mut byte).change_context(CmdError::Read)?;
                if n == 0 {
                    break;
                }
                if byte[0] == b'\n' {
                    break;
                }
                buf.push(byte[0]);
                if let Some(max) = max_bytes
                    && buf.len() >= max
                {
                    break;
                }
            }
        }
        SourceFd::RawFd(fd_arg) => {
            let fd = sys::ImportedFd::try_from(fd_arg).change_context(CmdError::Read)?;
            read_line_from_fd(|b: &mut [u8]| fd.read(b), &mut buf, &mut eof, max_bytes)?;
        }
        SourceFd::FdVar(_) => {
            if let Some(local) = fd_clone {
                read_line_from_fd(|b: &mut [u8]| local.read(b), &mut buf, &mut eof, max_bytes)?;
            }
        }
    }

    Ok((buf, eof))
}
