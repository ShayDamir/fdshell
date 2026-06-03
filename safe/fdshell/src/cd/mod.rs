use crate::vars::FdVars;
use std::ffi::CString;
use sys::errno::{EINVAL, ENOENT};
use sys::fcntl::{O_DIRECTORY, O_NOFOLLOW};
use sys::{LocalFd, ShortCStr};

pub fn cd(args: &[ShortCStr], fdvars: &mut FdVars) -> Result<(), i32> {
    let new_fd = match args.first() {
        None => cd_home()?,
        Some(arg) if arg.eq_bytes(b"-") => cd_var(&c"%OLDCWD".into(), fdvars)?,
        Some(arg) if arg.starts_with(b"%") => cd_var(arg, fdvars)?,
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
    let src = fdvars.resolve(&name).ok_or(ENOENT)?;
    src.try_clone()
}

fn cd_path(path: &ShortCStr) -> Result<LocalFd, i32> {
    let name = sys::RefCStr::from(path.clone());
    open_cwd_dir(&name)
}

fn open_cwd_dir(path: &std::ffi::CStr) -> Result<LocalFd, i32> {
    sys::openat2::open(path, O_DIRECTORY | O_NOFOLLOW)
}

fn move_cwd(fdvars: &mut FdVars, new_cwd: LocalFd) {
    if let Some(old) = fdvars.remove(&c"CWD".into()) {
        fdvars.insert(c"OLDCWD".into(), old);
    }
    fdvars.insert(c"CWD".into(), new_cwd);
}

#[cfg(test)]
mod tests;
