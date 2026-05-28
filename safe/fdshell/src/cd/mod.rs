use crate::vars::FdVars;
use std::ffi::CString;
use sys::errno::{EINVAL, ENOENT};
use sys::fcntl::{O_CLOEXEC, O_DIRECTORY, O_NOFOLLOW};
use sys::openat2::OpenHow;
use sys::{AtFd, LocalFd, ShortCStr};

pub fn cd(args: &[ShortCStr], fdvars: &mut FdVars) -> Result<(), i32> {
    let new_fd = match args.first() {
        None => cd_home()?,
        Some(arg) if arg.as_bytes() == b"-" => cd_var(&ShortCStr::from_static(c"%OLDCWD"), fdvars)?,
        Some(arg) if arg.as_bytes().starts_with(b"%") => cd_var(arg, fdvars)?,
        Some(path) => cd_path(path)?,
    };
    sys::fchdir::fchdir(&new_fd)?;
    move_cwd(fdvars, new_fd);
    Ok(())
}

fn cd_home() -> Result<LocalFd, i32> {
    let home = std::env::var_os("HOME").ok_or(ENOENT)?;
    let cs = CString::new(home.as_os_str().as_encoded_bytes()).map_err(|_| EINVAL)?;
    open_cwd_dir(&cs)
}

fn cd_var(arg: &ShortCStr, fdvars: &FdVars) -> Result<LocalFd, i32> {
    let name = arg.strip_prefix(b"%").ok_or(EINVAL)?;
    let src = fdvars.resolve(name.as_bytes()).ok_or(ENOENT)?;
    src.try_clone()
}

fn cd_path(path: &ShortCStr) -> Result<LocalFd, i32> {
    let cs = path.to_c_string();
    open_cwd_dir(&cs)
}

fn open_cwd_dir(path: &std::ffi::CStr) -> Result<LocalFd, i32> {
    let how = OpenHow {
        flags: O_DIRECTORY as u64 | O_CLOEXEC as u64 | O_NOFOLLOW as u64,
        mode: 0,
        resolve: 0,
    };
    sys::openat2::openat2(AtFd::cwd(), path, &how)
}

fn move_cwd(fdvars: &mut FdVars, new_cwd: LocalFd) {
    if let Some(old) = fdvars.remove(b"CWD") {
        fdvars.insert(ShortCStr::from_static(c"OLDCWD"), old);
    }
    fdvars.insert(ShortCStr::from_static(c"CWD"), new_cwd);
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests;
