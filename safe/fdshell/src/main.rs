#![forbid(unsafe_code)]

mod child;
mod launch;
mod vars;

use std::ffi::CString;
use sys::fcntl::{O_CLOEXEC, O_DIRECTORY};
use sys::openat2::OpenHow;
use sys::AtFd;

fn main() -> Result<(), i32> {
    sys::shellfd::reserve_shellfd()?;

    let mut vars = vars::Vars::new();

    let how = OpenHow {
        flags: O_DIRECTORY as u64 | O_CLOEXEC as u64,
        mode: 0,
        resolve: 0,
    };
    let cwd = sys::openat2::openat2(AtFd::cwd(), c".", &how)?;
    vars.insert(CString::from(c"CWD"), cwd);

    let cmd = child::Command::Builtin(CString::from(c"mkdirat"));
    let args = vec![
        CString::from(c"--mode"),
        CString::from(c"755"),
        CString::from(c"--dirfd"),
        CString::from(c"%CWD"),
        CString::from(c"foo"),
    ];
    let status = launch::launch(&vars, cmd, &args)?;
    println!("{status:?}");

    std::fs::remove_dir_all("foo").ok();
    Ok(())
}
