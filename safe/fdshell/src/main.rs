#![forbid(unsafe_code)]

mod capture;
mod child;
mod launch;
mod redirect;
mod resolve;
mod vars;

use std::ffi::CString;
use sys::fcntl::{O_CLOEXEC, O_DIRECTORY};
use sys::openat2::OpenHow;
use sys::siginfo::WaitStatus;
use sys::AtFd;

fn main() -> Result<(), i32> {
    sys::shellfd::reserve_shellfd()?;

    let mut fdvars = vars::FdVars::new();

    let how = OpenHow {
        flags: O_DIRECTORY as u64 | O_CLOEXEC as u64,
        mode: 0,
        resolve: 0,
    };
    let cwd = sys::openat2::openat2(AtFd::cwd(), c".", &how)?;
    fdvars.insert(CString::from(c"CWD"), cwd);

    let cmd = child::Command::Builtin(CString::from(c"mkdirat"));
    let args = vec![
        CString::from(c"--mode"),
        CString::from(c"755"),
        CString::from(c"--dirfd"),
        CString::from(c"%CWD"),
        CString::from(c"foo"),
    ];

    let captures = vec![capture::Capture {
        var: CString::from(c"foo"),
        tag: None,
        force: false,
    }];

    let (status, capture_fd) = launch::launch(&fdvars, cmd, &args, &[])?;
    println!("{status:?}");

    match status {
        WaitStatus::Exited(0) => {
            let entries = capture::do_captures(capture_fd, captures, &fdvars)?;
            for (var, fd) in entries {
                fdvars.insert(var, fd);
            }
        }
        other => std::process::exit(other.exit_code()),
    }

    for (name, raw) in fdvars.iter() {
        println!("  {:?} → fd {}", name, raw);
    }

    std::fs::remove_dir_all("foo").ok();
    Ok(())
}
