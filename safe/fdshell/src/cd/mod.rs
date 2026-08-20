use error_stack::{Report, ResultExt};

use crate::error::cd::CdError;
use crate::state::{FdVar, ShellState};
use sys::fcntl::{O_DIRECTORY, O_NOFOLLOW};
use sys::{LocalFd, Origin, Position, ShortCStr, Trace};

pub fn cd(
    args: &[ShortCStr],
    state: &mut ShellState,
    set_at: Position,
) -> Result<(), Report<CdError>> {
    let (new_fd, origin) = match args.first() {
        None => cd_home()?,
        Some(arg) if arg.eq_bytes(b"-") => cd_var(&c"%OLDCWD".into(), state)?,
        Some(arg) if arg.starts_with(b"%") => cd_var(arg, state)?,
        Some(path) => cd_path(path)?,
    };
    sys::fchdir::fchdir(&new_fd).change_context(CdError::CdPathOpen)?;
    move_cwd(state, new_fd, origin, set_at);
    Ok(())
}

fn cd_home() -> Result<(LocalFd, Origin), Report<CdError>> {
    let home = sys::env::getenv(c"HOME").ok_or(CdError::HomeNotSet)?;
    let fd = open_dir(&home)?;
    Ok((fd, Origin::EnvVar(ShortCStr::from(c"HOME"))))
}

fn cd_var(arg: &ShortCStr, state: &ShellState) -> Result<(LocalFd, Origin), Report<CdError>> {
    let name = arg.strip_prefix(b"%").ok_or(CdError::Never)?;
    let src = state.fds.get(&name).ok_or(CdError::FdNotSet)?;
    let fd = src.fd.try_clone().change_context(CdError::CdPathOpen)?;
    Ok((fd, src.trace.origin.clone()))
}

fn cd_path(path: &ShortCStr) -> Result<(LocalFd, Origin), Report<CdError>> {
    let fd = open_dir(path)?;
    Ok((fd, Origin::File(path.clone())))
}

fn open_dir(path: &ShortCStr) -> Result<LocalFd, Report<CdError>> {
    sys::openat2::open(path.export(), O_DIRECTORY | O_NOFOLLOW).change_context(CdError::CdPathOpen)
}

fn move_cwd(state: &mut ShellState, new_cwd: LocalFd, origin: Origin, set_at: Position) {
    let cwd_key: ShortCStr = c"CWD".into();
    if let Some(old) = state.fds.remove(&cwd_key) {
        state.fds.insert(c"OLDCWD".into(), old);
    }
    state.fds.insert(
        cwd_key,
        FdVar {
            fd: new_cwd,
            trace: Trace::at(set_at, origin),
        },
    );
}

#[cfg(test)]
mod tests;
