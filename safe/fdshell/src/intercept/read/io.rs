use crate::error::cmd::CmdError;
use crate::error::read::ReadError;
use error_stack::{Report, ResultExt};
use std::io::Read as _;

use super::flags::SourceFd;

pub(crate) fn read_line(
    source: &SourceFd,
    fd_clone: Option<&sys::LocalFd>,
    max_bytes: Option<usize>,
) -> Result<(Vec<u8>, bool), Report<CmdError>> {
    let mut buf = Vec::new();
    let mut eof = false;

    match source {
        SourceFd::Stdin => {
            for byte in std::io::stdin().lock().bytes() {
                let b = byte
                    .change_context(ReadError::Io)
                    .change_context(CmdError::Read)?;
                if b == b'\n' {
                    break;
                }
                buf.push(b);
                if let Some(max) = max_bytes
                    && buf.len() >= max
                {
                    break;
                }
            }
        }
        SourceFd::RawFd(fd) => {
            let mut temp = [0u8; 4096];
            loop {
                let mut done = false;
                match sys::ImportedFd::read_from_raw(*fd, &mut temp) {
                    Ok(n) => match n {
                        1.. => {
                            for &b in temp
                                .get(..n as usize)
                                .ok_or(ReadError::Io)
                                .change_context(CmdError::Read)?
                            {
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
                        }
                        0 => {
                            eof = true;
                            break;
                        }
                        _ => {
                            return Err(Report::new(ReadError::Io)
                                .attach_opaque("read returned negative value")
                                .change_context(CmdError::Read));
                        }
                    },
                    Err(e) => {
                        return Err(Report::new(e)
                            .change_context(ReadError::Io)
                            .change_context(CmdError::Read));
                    }
                }
                if eof || done {
                    break;
                }
            }
        }
        SourceFd::FdVar(_) => {
            if let Some(local) = fd_clone {
                super::read_from_fd::read_from_local_fd(local, &mut buf, &mut eof, max_bytes)?;
            }
        }
    }

    Ok((buf, eof))
}
