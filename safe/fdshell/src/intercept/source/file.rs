//! `source`'s file half: open the path, read its content (capped), and run
//! it as a script in this shell.

use alloc::vec::Vec;
use error_stack::{Report, ResultExt, bail};
use sys::Origin;
use sys::Position;
use sys::ScriptText;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

use crate::error::cmd::CmdError;
use crate::loop_control::LoopControl;
use crate::state::ShellState;

/// Open `path`, read its content, and run it as a script in this shell.
pub(super) fn run_sourced(
    path: &ShortCStr,
    cell: &ForkCell<ShellState>,
) -> Result<Option<LoopControl>, Report<CmdError>> {
    let fd = sys::openat2::open(path.export(), sys::fcntl::O_RDONLY)
        .change_context(CmdError::SourceOpen)?;
    let content = read_to_end(&fd, crate::cmd_subst::MAX_CAPTURED)?;
    let data = ShortCStr::from_vec(content).change_context(CmdError::SourceNul)?;
    let script = ScriptText::new(data, Position::new(1, 1), Origin::File(path.clone()));
    // Count each source level toward the nesting cap: a self-sourcing file
    // recurses through run_script and would otherwise overflow the stack.
    crate::nest::deeper(cell, CmdError::NestingTooDeep, || {
        crate::script::run_script(&script, cell)
    })
}

/// Read `fd` to EOF, failing with [`CmdError::SourceTooLarge`] once `limit`
/// bytes would be read.
pub(super) fn read_to_end(fd: &sys::LocalFd, limit: usize) -> Result<Vec<u8>, Report<CmdError>> {
    let mut content = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        let n = fd.read(&mut buf).change_context(CmdError::SourceRead)?;
        if n == 0 {
            break;
        }
        let slice = buf.get(..n).ok_or(CmdError::Never)?;
        if content.len() + slice.len() > limit {
            bail!(CmdError::SourceTooLarge);
        }
        content.extend_from_slice(slice);
    }
    Ok(content)
}
