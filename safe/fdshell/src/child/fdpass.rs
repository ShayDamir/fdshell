use crate::error::fdpass::FdPassError;
use crate::state::ShellState;
use error_stack::{Report, ResultExt};
use sys::ShortCStr;

pub fn dispatch(
    name: &[u8],
    args: &[ShortCStr],
    state: &ShellState,
) -> Option<Result<(), Report<FdPassError>>> {
    match name {
        b"import_fd" => Some(import_fd(args)),
        b"export_fd" => Some(export_fd(args, state)),
        _ => None,
    }
}

fn import_fd(args: &[ShortCStr]) -> Result<(), Report<FdPassError>> {
    let raw = args.first().ok_or(FdPassError::MissingArg)?;
    let fd = sys::ImportedFd::try_from(raw).change_context(FdPassError::InvalidName)?;
    let local = fd
        .try_into_local()
        .change_context(FdPassError::SendFailed)?;
    sys::shellfd::send_fd(&local, c"import_fd").change_context(FdPassError::SendFailed)
}

pub(crate) fn export_fd(args: &[ShortCStr], state: &ShellState) -> Result<(), Report<FdPassError>> {
    let (vname, tag) = match args {
        [a] => {
            let v = a.strip_prefix(b"%").ok_or(FdPassError::InvalidName)?;
            let tag = sys::RefCStr::from(v.clone());
            (v, tag)
        }
        [t, v] => {
            if t.contains(b'%') {
                return Err(Report::new(FdPassError::InvalidName));
            }
            let tag = sys::RefCStr::from(t.clone());
            let v = v.strip_prefix(b"%").ok_or(FdPassError::InvalidName)?;
            (v, tag)
        }
        _ => return Err(Report::new(FdPassError::MissingArg)),
    };
    let fd = state.fds.get(&vname).ok_or(FdPassError::NotFound)?;
    sys::shellfd::send_fd(fd, &tag).change_context(FdPassError::SendFailed)
}
