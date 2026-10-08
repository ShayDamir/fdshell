use crate::error::cmd::CmdError;
use error_stack::{Report, ResultExt};
use sys::SyscallError;

use super::line::Line;

/// Read from `read` into `line` until the line is finished or the input ends.
pub(crate) fn read_line_from_fd(
    mut read: impl FnMut(&mut [u8]) -> Result<usize, SyscallError>,
    line: &mut Line,
) -> Result<(), Report<CmdError>> {
    let mut temp = [0u8; 4096];
    while !line.finished() {
        let n = read(&mut temp).change_context(CmdError::Read)?;
        if n == 0 {
            line.end_eof();
            break;
        }
        for &b in temp.get(..n).ok_or(CmdError::Never)? {
            line.feed(b);
        }
    }
    Ok(())
}
