#![forbid(unsafe_code)]

mod capture;
mod cd;
mod child;
mod exec;
mod launch;
mod parse;
mod redirect;
mod repl;
mod resolve;
mod vars;

use std::io::Write;

use sys::fcntl::{O_CLOEXEC, O_DIRECTORY};
use sys::openat2::OpenHow;
use sys::{AtFd, LocalFd, ShortCStr};

enum FdShellMode {
    Nested(LocalFd),
    Standalone(LocalFd),
}

fn detect_nested() -> Option<sys::ImportedFd> {
    let cookie = std::env::var("FDSHELL_CAPTURE").ok();
    let pid = cookie.and_then(|s| s.parse::<u32>().ok())?;
    if pid != std::process::id() {
        return None;
    }
    sys::ImportedFd::from_bytes(sys::shellfd::SHELLFD_STR).ok()
}

fn init_shellfd() -> Result<FdShellMode, i32> {
    if let Some(dupfd) = detect_nested() {
        let fd = dupfd.try_into_local()?;
        Ok(FdShellMode::Nested(fd))
    } else {
        let fd = sys::shellfd::reserve_shellfd()?;
        Ok(FdShellMode::Standalone(fd))
    }
}

fn main() -> Result<(), i32> {
    let _mode = init_shellfd()?;
    let mut fdvars = vars::FdVars::new();
    let stdin = std::io::stdin();
    let mut buf = String::new();
    let how = OpenHow {
        flags: O_DIRECTORY as u64 | O_CLOEXEC as u64,
        mode: 0,
        resolve: 0,
    };
    let cwd = sys::openat2::openat2(AtFd::cwd(), c".", &how)?;
    fdvars.insert(ShortCStr::from_static(c"CWD"), cwd);
    loop {
        buf.clear();
        print!("fdshell> ");
        std::io::stdout().flush().ok();
        let n = stdin.read_line(&mut buf).map_err(|_| sys::errno::EIO)?;
        if n == 0 {
            println!();
            break;
        }
        let line = buf.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line == "exit" || line == "quit" {
            break;
        }
        repl::handle(line, &mut fdvars)?;
    }
    Ok(())
}
