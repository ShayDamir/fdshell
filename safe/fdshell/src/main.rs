#![forbid(unsafe_code)]

mod capture;
mod cd;
mod child;
mod exec;
mod init;
mod launch;
mod parse;
mod pipeline;
mod redirect;
mod repl;
mod replacer;
mod resolve;
mod run;
mod vars;

use std::io::Write;
use sys::ShortCStr;
use sys::fcntl::O_DIRECTORY;
use sys::siginfo::WaitStatus;

fn main() -> Result<(), i32> {
    let _mode = crate::init::init_shellfd()?;
    sys::umask::init();
    let mut fdvars = vars::FdVars::new();
    let cwd = sys::openat2::open(c".", O_DIRECTORY)?;
    fdvars.insert(ShortCStr::from_static(c"CWD"), cwd);
    let mut last_status = WaitStatus::Exited(0);
    let mut args = std::env::args().skip(1);
    if let Some(arg) = args.next() {
        if arg == "-c" {
            let cmd = args.next().ok_or(sys::errno::EINVAL)?;
            let code = repl::exec_cmd(&cmd, &mut fdvars, &mut last_status)?;
            if code != 0 {
                std::process::exit(code);
            }
            return Ok(());
        }
        return Err(sys::errno::EINVAL);
    }
    let stdin = std::io::stdin();
    let mut buf = String::new();
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
        repl::handle(line, &mut fdvars, &mut last_status)?;
    }
    Ok(())
}
