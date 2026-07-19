use crate::error::fdpass::FdPassError;
use crate::state::ShellState;
use error_stack::{Report, ResultExt, bail, ensure};
use sys::ShortCStr;

pub fn dispatch(
    name: &[u8],
    args: &[ShortCStr],
    state: &ShellState,
) -> Option<Result<i32, Report<FdPassError>>> {
    match name {
        b"import_fd" => Some(import_fd(args, state)),
        b"export_fd" => Some(export_fd(args, state)),
        _ => None,
    }
}

fn import_fd(args: &[ShortCStr], state: &ShellState) -> Result<i32, Report<FdPassError>> {
    let raw = args.first().ok_or(FdPassError::MissingArg)?;
    let fd = sys::ImportedFd::try_from(raw).change_context(FdPassError::InvalidName)?;
    let local = fd
        .try_into_local()
        .change_context(FdPassError::SendFailed)?;
    let sock = state.shell_sock.as_ref().ok_or(FdPassError::SendFailed)?;
    sys::shellfd::send_fd(sock, &local, c"import_fd").change_context(FdPassError::SendFailed)?;
    Ok(0)
}

pub(crate) fn export_fd(
    args: &[ShortCStr],
    state: &ShellState,
) -> Result<i32, Report<FdPassError>> {
    let (vname, tag) = match args {
        [a] => {
            let v = a.strip_prefix(b"%").ok_or(FdPassError::InvalidName)?;
            let tag = v.export();
            (v, tag)
        }
        [t, v] => {
            ensure!(!t.contains(b'%'), FdPassError::InvalidName);
            let tag = t.export();
            let v = v.strip_prefix(b"%").ok_or(FdPassError::InvalidName)?;
            (v, tag)
        }
        _ => bail!(FdPassError::MissingArg),
    };
    let fd = state.fds.get(&vname).ok_or(FdPassError::NotFound)?;
    let sock = state.shell_sock.as_ref().ok_or(FdPassError::SendFailed)?;
    sys::shellfd::send_fd(sock, fd, &tag).change_context(FdPassError::SendFailed)?;
    Ok(0)
}
