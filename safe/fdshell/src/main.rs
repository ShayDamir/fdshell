#![forbid(unsafe_code)]

mod capture;
mod cd;
mod child;
mod cond;
mod exec;
mod init;
mod intercept;
mod launch;
mod parse;
mod pipeline;
mod postlaunch;
mod redirect;
mod repl;
mod replacer;
mod resolve;
mod run;
mod script;
mod state;
mod task;

#[cfg(test)]
mod tests;

use std::io::{BufRead, Write};
use sys::fcntl::O_DIRECTORY;

fn main() -> Result<(), i32> {
    let _mode = crate::init::init_shellfd()?;
    sys::umask::init();
    let mut state = state::ShellState::new();
    let cwd = sys::openat2::open(c".", O_DIRECTORY)?;
    state.fds.insert(c"CWD".into(), cwd);
    let mut args = std::env::args().skip(1);
    if let Some(arg) = args.next() {
        if arg == "-c" {
            let cmd = args.next().ok_or(sys::errno::EINVAL)?;
            let code = repl::exec_cmd(cmd.as_bytes(), &mut state)?;
            if code != 0 {
                std::process::exit(code);
            }
            return Ok(());
        }
        return Err(sys::errno::EINVAL);
    }
    let mut buf = Vec::<u8>::new();
    loop {
        buf.clear();
        print!("fdshell> ");
        std::io::stdout().flush().ok();
        let n = std::io::stdin()
            .lock()
            .read_until(b'\n', &mut buf)
            .map_err(|_| sys::errno::EIO)?;
        if n == 0 {
            println!();
            break;
        }
        let line = buf.trim_ascii();
        if line.is_empty() || line.starts_with(b"#") {
            continue;
        }
        repl::handle(line, &mut state)?;
    }
    Ok(())
}
