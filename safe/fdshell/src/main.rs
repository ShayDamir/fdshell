#![forbid(unsafe_code)]

mod app;
mod capture;
mod cd;
mod child;
mod cmd_subst;
mod cond;
mod error;
mod exec;
mod expand;
mod exports;
mod if_exec;
mod init;
mod intercept;
mod launch;
mod loop_;
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
mod substitute;
mod task;

#[cfg(test)]
mod tests;

use crate::error::LegacyError;
use app::AppError;
use error_stack::{Report, ResultExt, bail};
use std::io::{BufRead, Write};
use sys::fcntl::O_DIRECTORY;

fn main() -> Result<(), Report<AppError>> {
    Report::install_debug_hook::<std::panic::Location>(
        |_location: &std::panic::Location, _ctx: &mut HookContext<std::panic::Location>| {
            #[cfg(debug_assertions)]
            _ctx.push_body(format!("at {}", _location));
        },
    );

    Report::install_debug_hook::<crate::error::parse::ParseError>(|_, _| {});

    // Install debug hook for ParsePosition to show input line with caret
    use crate::error::parse::ParsePosition;
    use error_stack::fmt::HookContext;
    Report::install_debug_hook::<ParsePosition>(
        |ParsePosition { pos, input }, ctx: &mut HookContext<ParsePosition>| {
            let pos = *pos;
            let input = input.as_deref().unwrap_or(&[]);
            if input.is_empty() {
                ctx.push_body(format!("parse error at byte position {}", pos));
            } else {
                let line_start = input
                    .get(..pos)
                    .and_then(|prefix| prefix.iter().rposition(|&b| b == b'\n').map(|p| p + 1))
                    .unwrap_or(0);
                let line_end = input
                    .get(pos..)
                    .and_then(|suffix| suffix.iter().position(|&b| b == b'\n').map(|p| pos + p))
                    .unwrap_or(input.len());
                let line = input
                    .get(line_start..line_end)
                    .and_then(|s| std::str::from_utf8(s).ok())
                    .unwrap_or("?");
                let caret_col = pos - line_start;
                let rest = input.get(line_start..).unwrap_or(&[]);
                let local_pos = pos - line_start;
                let mut caret_len = 1;
                for &kw in &[
                    b"if" as &[u8],
                    b"fi",
                    b"then",
                    b"else",
                    b"elif",
                    b"for",
                    b"while",
                    b"until",
                    b"done",
                ] {
                    let kw_len = kw.len();
                    if rest.get(local_pos..local_pos + kw_len) == Some(kw) {
                        let after = local_pos + kw_len;
                        if after >= rest.len()
                            || rest.get(after).is_some_and(|&b| b.is_ascii_whitespace())
                        {
                            caret_len = kw_len;
                            break;
                        }
                    }
                }
                ctx.push_body(line.to_string());
                ctx.push_body(caret_line(caret_col, caret_len));
            }
        },
    );

    let _mode = crate::init::init_shellfd()
        .map_err(LegacyError)
        .change_context(AppError::Init)?;
    sys::umask::init();
    let state = sys::fork_cell::ForkCell::new(state::ShellState::new());
    {
        let mut state = state.borrow_mut().change_context(AppError::Borrow)?;
        let cwd = sys::openat2::open(c".", O_DIRECTORY)
            .map_err(|e| LegacyError(e.into()))
            .change_context(AppError::Cwd)?;
        state.fds.insert(c"CWD".into(), cwd);
    }
    let mut args = std::env::args().skip(1);
    if let Some(arg) = args.next() {
        if arg == "-c" {
            let cmd = args.next().ok_or(AppError::Usage)?;
            match repl::exec_cmd(cmd.as_bytes(), &state) {
                Ok(code) => {
                    if code != 0 {
                        std::process::exit(code);
                    }
                    return Ok(());
                }
                Err(info) => {
                    eprintln!("{info:?}");
                    std::process::exit(1);
                }
            }
        }
        bail!(AppError::Usage);
    }
    let mut buf = Vec::new();
    loop {
        buf.clear();
        print!("fdshell> ");
        std::io::stdout().flush().change_context(AppError::Read)?;
        let n = std::io::stdin()
            .lock()
            .read_until(b'\n', &mut buf)
            .change_context(AppError::Read)?;
        if n == 0 {
            println!();
            break;
        }
        let line = buf.trim_ascii();
        if line.is_empty() || line.starts_with(b"#") {
            continue;
        }
        if let Err(err) = repl::handle(line, &state) {
            eprintln!("{err:?}");
        }
    }
    Ok(())
}

fn caret_line(col: usize, len: usize) -> String {
    let mut s = String::with_capacity(col + len);
    for _ in 0..col {
        s.push(' ');
    }
    match len {
        0 => {}
        1 => s.push('^'),
        _ => {
            s.push('^');
            for _ in 2..len {
                s.push('~');
            }
            s.push('^');
        }
    }
    s
}
