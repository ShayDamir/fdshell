#![forbid(unsafe_code)]

use crate::vars::FdVars;
use std::ffi::CString;
use sys::errno::{EINVAL, ENOENT};
use sys::fcntl::{O_CLOEXEC, O_DIRECTORY, O_NOFOLLOW};
use sys::openat2::OpenHow;
use sys::{AtFd, Fd, ShortCStr};

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

fn cd_home() -> Result<Fd, i32> {
    let home = std::env::var_os("HOME").ok_or(ENOENT)?;
    let cs = CString::new(home.as_os_str().as_encoded_bytes()).map_err(|_| EINVAL)?;
    open_cwd_dir(&cs)
}

fn cd_var(arg: &ShortCStr, fdvars: &FdVars) -> Result<Fd, i32> {
    let name = arg.strip_prefix(b"%").ok_or(EINVAL)?;
    let src = fdvars.resolve(name.as_bytes()).ok_or(ENOENT)?;
    src.try_clone()
}

fn cd_path(path: &ShortCStr) -> Result<Fd, i32> {
    let cs = path.to_c_string();
    open_cwd_dir(&cs)
}

fn open_cwd_dir(path: &std::ffi::CStr) -> Result<Fd, i32> {
    let how = OpenHow {
        flags: O_DIRECTORY as u64 | O_CLOEXEC as u64 | O_NOFOLLOW as u64,
        mode: 0,
        resolve: 0,
    };
    sys::openat2::openat2(AtFd::cwd(), path, &how)
}

fn move_cwd(fdvars: &mut FdVars, new_cwd: Fd) {
    if let Some(old) = fdvars.remove(b"CWD") {
        fdvars.insert(ShortCStr::from_static(c"OLDCWD"), old);
    }
    fdvars.insert(ShortCStr::from_static(c"CWD"), new_cwd);
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use sys::siginfo::WaitStatus;

    fn child_test(f: impl FnOnce()) {
        let (_, pidfd_opt) = sys::fork_pidfd::fork_pidfd().unwrap();
        match pidfd_opt {
            None => {
                f();
                std::process::exit(42);
            }
            Some(pidfd) => {
                let status = sys::wait_pidfd::wait_pidfd(&pidfd).unwrap();
                match status {
                    WaitStatus::Exited(42) => {}
                    other => panic!("unexpected {other:?}"),
                }
            }
        }
    }

    #[test]
    fn cd_to_tmp() {
        child_test(|| {
            let mut v = FdVars::new();
            let tmp = ShortCStr::from_static(c"/tmp");
            cd(&[tmp], &mut v).unwrap();
            let cwd = v.resolve(b"CWD").unwrap();
            cwd.verify().unwrap();
        });
    }

    #[test]
    fn cd_to_home() {
        let home_exists =
            std::env::var_os("HOME").is_some_and(|p| std::path::Path::new(&p).exists());
        if home_exists {
            child_test(|| {
                let mut v = FdVars::new();
                cd(&[], &mut v).unwrap();
                let cwd = v.resolve(b"CWD").unwrap();
                cwd.verify().unwrap();
            });
        } else {
            child_test(|| {
                let mut v = FdVars::new();
                let e = cd(&[], &mut v).unwrap_err();
                assert_eq!(e, sys::errno::ENOENT);
            });
        }
    }

    #[test]
    fn cd_to_self() {
        child_test(|| {
            let mut v = FdVars::new();
            let tmp = ShortCStr::from_static(c"/tmp");
            cd(&[tmp], &mut v).unwrap();
            let cwd_fd = v.resolve(b"CWD").unwrap().try_clone().unwrap();
            v.insert(ShortCStr::from_static(c"CWD"), cwd_fd);
            cd(&[ShortCStr::from_static(c"%CWD")], &mut v).unwrap();
            let cwd = v.resolve(b"CWD").unwrap();
            cwd.verify().unwrap();
        });
    }

    #[test]
    fn cd_missing_path() {
        child_test(|| {
            let mut v = FdVars::new();
            let bad = ShortCStr::from_static(c"/nonexistent-cd-test-xxxxxxxx");
            let e = cd(&[bad], &mut v).unwrap_err();
            assert_eq!(e, sys::errno::ENOENT);
        });
    }

    #[test]
    fn cd_missing_var() {
        child_test(|| {
            let mut v = FdVars::new();
            let bad = ShortCStr::from_static(c"%NONEXISTENT");
            let e = cd(&[bad], &mut v).unwrap_err();
            assert_eq!(e, sys::errno::ENOENT);
        });
    }

    #[test]
    fn cd_dash_switches_to_oldpwd() {
        child_test(|| {
            let mut v = FdVars::new();
            let tmp = ShortCStr::from_static(c"/tmp");
            cd(&[tmp], &mut v).unwrap();
            let root = ShortCStr::from_static(c"/");
            cd(&[root], &mut v).unwrap();
            let dash = ShortCStr::from_static(c"-");
            cd(&[dash], &mut v).unwrap();
            let cwd = v.resolve(b"CWD").unwrap();
            cwd.verify().unwrap();
            let old = v.resolve(b"OLDCWD").unwrap();
            old.verify().unwrap();
        });
    }

    #[test]
    fn cd_move_cwd_to_oldcwd() {
        child_test(|| {
            let mut v = FdVars::new();
            let tmp = ShortCStr::from_static(c"/tmp");
            cd(&[tmp], &mut v).unwrap();
            assert!(v.resolve(b"CWD").is_some());
            assert!(v.resolve(b"OLDCWD").is_none());
            let root = ShortCStr::from_static(c"/");
            cd(&[root], &mut v).unwrap();
            assert!(v.resolve(b"CWD").is_some());
            assert!(v.resolve(b"OLDCWD").is_some());
        });
    }
}
