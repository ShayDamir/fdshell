#![forbid(unsafe_code)]

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

    let name = CString::from(c"CWD");
    vars.insert(name, cwd);

    for (tag, num) in vars.iter() {
        let s = tag.to_str().unwrap_or("?");
        println!("{s} -> {num}");
    }
    Ok(())
}
