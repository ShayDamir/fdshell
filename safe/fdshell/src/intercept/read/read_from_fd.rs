use crate::error::cmd::CmdError;
use crate::error::read::ReadError;
use error_stack::{Report, ResultExt};

pub(crate) fn read_from_local_fd(
    fd: &sys::LocalFd,
    buf: &mut Vec<u8>,
    eof: &mut bool,
    max_bytes: Option<usize>,
) -> Result<(), Report<CmdError>> {
    let mut temp = [0u8; 4096];
    loop {
        let mut done = false;
        match fd.read(&mut temp) {
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
                    *eof = true;
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
        if *eof || done {
            break;
        }
    }
    Ok(())
}
