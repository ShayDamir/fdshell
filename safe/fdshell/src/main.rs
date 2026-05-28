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

use sys::AtFd;
use sys::ShortCStr;
use sys::fcntl::{O_CLOEXEC, O_DIRECTORY};
use sys::openat2::OpenHow;

fn main() -> Result<(), i32> {
    sys::shellfd::reserve_shellfd()?;
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
