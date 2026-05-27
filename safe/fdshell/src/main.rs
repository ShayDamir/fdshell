#![forbid(unsafe_code)]

mod capture;
mod cd;
mod child;
mod exec;
mod launch;
mod parse;
mod redirect;
mod resolve;
mod vars;

use std::io::Write;

use sys::AtFd;
use sys::ShortCStr;
use sys::errno::EINVAL;
use sys::fcntl::{O_CLOEXEC, O_DIRECTORY};
use sys::openat2::OpenHow;
use sys::siginfo::WaitStatus;

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
        match parse::parse(line)? {
            parse::ParsedLine::Cmd(cmdline) => {
                if cmdline.command.as_bytes() == b"cd" {
                    if cmdline.builtin
                        || !cmdline.captures.is_empty()
                        || !cmdline.redirects.is_empty()
                    {
                        return Err(EINVAL);
                    }
                    cd::cd(&cmdline.args, &mut fdvars)?;
                    continue;
                }
                let (status, capture_fd) = launch::launch(&fdvars, &cmdline)?;
                match status {
                    WaitStatus::Exited(0) => {
                        let entries = capture::do_captures(capture_fd, cmdline.captures, &fdvars)?;
                        for (var, fd) in entries {
                            fdvars.insert(var, fd);
                        }
                    }
                    WaitStatus::Exited(n) => eprintln!("exit code: {n}"),
                    _ => eprintln!("{status:?}"),
                }
            }
            parse::ParsedLine::Assign { var, value } => {
                let src = fdvars.resolve(value.as_bytes()).ok_or(EINVAL)?;
                fdvars.insert(var, src.try_clone()?);
            }
            parse::ParsedLine::Unset(var) => {
                fdvars.remove(var.as_bytes());
            }
        }
    }
    Ok(())
}
