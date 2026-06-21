use crate::exec;
use crate::state::ShellState;
use std::ffi::CStr;
use sys::ShortCStr;

pub fn dispatch_builtin(
    name: ShortCStr,
    refs: &[&CStr],
    args: &[ShortCStr],
    state: &ShellState,
) -> Result<(), i32> {
    match name.as_bytes()? {
        b"true" => Ok(()),
        b"false" => Err(1),
        b"pwd" => {
            let p = match std::env::current_dir() {
                Ok(p) => p,
                Err(_) => return Err(sys::errno::EINVAL),
            };
            println!("{}", p.display());
            Ok(())
        }
        b"fchmod" => builtins::fchmod::parse::fchmod_parse(refs)
            .and_then(|cfg| builtins::fchmod::fchmod_exec(&cfg)),
        b"echo" => {
            use std::io::Write;
            let mut lock = std::io::stdout().lock();
            for (i, arg) in refs.iter().enumerate() {
                if i > 0 {
                    let _ = lock.write_all(b" ");
                }
                let _ = lock.write_all(arg.to_bytes());
            }
            let _ = lock.write_all(b"\n");
            Ok(())
        }
        b"pipe" => builtins::pipe::parse::pipe_parse(refs)
            .and_then(|cfg| builtins::pipe::pipe_exec(cfg.flags)),
        b"mkdirat" => builtins::mkdirat::parse::mkdirat_parse(refs)
            .and_then(|cfg| builtins::mkdirat::mkdirat_exec(&cfg)),
        b"openat2" => builtins::openat2::parse::openat2_parse(refs)
            .and_then(|cfg| builtins::openat2::openat2_exec(&cfg)),
        b"renameat2" => builtins::renameat2::parse::renameat2_parse(refs)
            .and_then(|cfg| builtins::renameat2::renameat2_exec(&cfg)),
        b"exec_fd" => {
            let raw0 = args.first().ok_or(sys::errno::EINVAL)?;
            let varname = raw0.strip_prefix(b"%").ok_or(sys::errno::EINVAL)?;
            let fd = state.fds.get(&varname).ok_or(sys::errno::EINVAL)?;
            let exports: Vec<(sys::ShortCStr, Vec<u8>)> = state
                .exports
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            match exec::exec_fd(fd, refs.get(1..).ok_or(sys::errno::EINVAL)?, &exports) {
                Ok(()) => Ok(()),
                Err(report) => Err(report.current_context().exit_code()),
            }
        }
        b"exec_at" => {
            let raw0 = args.first().ok_or(sys::errno::EINVAL)?;
            let varname = raw0.strip_prefix(b"%").ok_or(sys::errno::EINVAL)?;
            let dirfd = state.fds.get(&varname).ok_or(sys::errno::EINVAL)?;
            let pathname = args.get(1).ok_or(sys::errno::EINVAL)?;
            let pathname = sys::RefCStr::from(pathname.clone());
            // execveat with a relative pathname rejects dirfds that have FD_CLOEXEC set.
            // Create a non-CLOEXEC copy via export().
            let non_cloexec = dirfd.export()?;
            let exports: Vec<(sys::ShortCStr, Vec<u8>)> = state
                .exports
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            match exec::exec_at(
                non_cloexec.at(),
                &pathname,
                refs.get(2..).ok_or(sys::errno::EINVAL)?,
                &exports,
            ) {
                Ok(()) => Ok(()),
                Err(report) => Err(report.current_context().exit_code()),
            }
        }
        b"resolve" => {
            let name = refs.first().ok_or(sys::errno::EINVAL)?;
            let fd = match exec::resolve_path(name) {
                Ok(fd) => fd,
                Err(report) => return Err(report.current_context().exit_code()),
            };
            sys::shellfd::send_fd(&fd, c"resolve").ok();
            Ok(())
        }
        name_bytes => match crate::child::fdpass::dispatch(name_bytes, args, state) {
            Some(result) => match result {
                Ok(v) => Ok(v),
                Err(report) => Err(match report.current_context() {
                    crate::error::fdpass::FdPassError::SendFailed => sys::errno::EIO,
                    crate::error::fdpass::FdPassError::NotFound
                    | crate::error::fdpass::FdPassError::InvalidName
                    | crate::error::fdpass::FdPassError::MissingArg => sys::errno::EINVAL,
                }),
            },
            None => Err(sys::errno::ENOSYS),
        },
    }
}
