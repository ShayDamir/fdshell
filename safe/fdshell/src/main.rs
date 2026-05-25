#![forbid(unsafe_code)]

mod capture;
mod child;
mod launch;
mod parse;
mod redirect;
mod resolve;
mod vars;

use sys::AtFd;
use sys::ShortCStr;
use sys::errno::EINVAL;
use sys::fcntl::{O_CLOEXEC, O_DIRECTORY};
use sys::openat2::OpenHow;
use sys::siginfo::WaitStatus;

fn main() -> Result<(), i32> {
    sys::shellfd::reserve_shellfd()?;

    let mut fdvars = vars::FdVars::new();

    let how = OpenHow {
        flags: O_DIRECTORY as u64 | O_CLOEXEC as u64,
        mode: 0,
        resolve: 0,
    };
    let cwd = sys::openat2::openat2(AtFd::cwd(), c".", &how)?;
    fdvars.insert(ShortCStr::from_static(c"CWD"), cwd);

    let parsed = parse::parse("builtin mkdirat --mode 755 --dirfd %CWD foo %>%foo")?;

    match parsed {
        parse::ParsedLine::Cmd(cmdline) => {
            let (status, capture_fd) = launch::launch(&fdvars, &cmdline)?;
            println!("{status:?}");

            match status {
                WaitStatus::Exited(0) => {
                    let entries = capture::do_captures(capture_fd, cmdline.captures, &fdvars)?;
                    for (var, fd) in entries {
                        fdvars.insert(var, fd);
                    }
                }
                other => std::process::exit(other.exit_code()),
            }
        }
        parse::ParsedLine::Assign { var, value } => {
            let src = fdvars.resolve(value.as_c_str()).ok_or(EINVAL)?;
            fdvars.insert(var, src.try_clone_any()?);
        }
        parse::ParsedLine::Unset(var) => {
            fdvars.remove(var.as_c_str());
        }
    }

    for (name, raw) in fdvars.iter() {
        println!("  {:?} → fd {}", name, raw);
    }

    std::fs::remove_dir_all("foo").ok();
    Ok(())
}
