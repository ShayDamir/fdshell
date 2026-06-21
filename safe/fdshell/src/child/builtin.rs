use crate::exec;
use crate::state::ShellState;
use builtins::error::BuiltinError;
use std::ffi::CStr;
use sys::ShortCStr;

pub fn dispatch_builtin(
    name: ShortCStr,
    refs: &[&CStr],
    args: &[ShortCStr],
    state: &ShellState,
) -> Result<i32, BuiltinError> {
    // Validate ShortCStr internal state before dispatching.
    let _ = name.as_bytes().map_err(|_| BuiltinError::InvalidArgument)?;
    if name.eq_bytes(b"true") {
        Ok(0)
    } else if name.eq_bytes(b"false") {
        Ok(1)
    } else if name.eq_bytes(b"pwd") {
        let p = match std::env::current_dir() {
            Ok(p) => p,
            Err(_) => return Err(BuiltinError::InvalidArgument),
        };
        println!("{}", p.display());
        Ok(0)
    } else if name.eq_bytes(b"fchmod") {
        builtins::fchmod::parse::fchmod_parse(refs)
            .and_then(|cfg| builtins::fchmod::fchmod_exec(&cfg))
            .map(|()| 0)
    } else if name.eq_bytes(b"echo") {
        use std::io::Write;
        let mut lock = std::io::stdout().lock();
        for (i, arg) in refs.iter().enumerate() {
            if i > 0 {
                let _ = lock.write_all(b" ");
            }
            let _ = lock.write_all(arg.to_bytes());
        }
        let _ = lock.write_all(b"\n");
        Ok(0)
    } else if name.eq_bytes(b"pipe") {
        builtins::pipe::parse::pipe_parse(refs)
            .and_then(|cfg| builtins::pipe::pipe_exec(cfg.flags))
            .map(|()| 0)
    } else if name.eq_bytes(b"mkdirat") {
        builtins::mkdirat::parse::mkdirat_parse(refs)
            .and_then(|cfg| builtins::mkdirat::mkdirat_exec(&cfg))
            .map(|()| 0)
    } else if name.eq_bytes(b"openat2") {
        builtins::openat2::parse::openat2_parse(refs)
            .and_then(|cfg| builtins::openat2::openat2_exec(&cfg))
            .map(|()| 0)
    } else if name.eq_bytes(b"renameat2") {
        builtins::renameat2::parse::renameat2_parse(refs)
            .and_then(|cfg| builtins::renameat2::renameat2_exec(&cfg))
            .map(|()| 0)
    } else if name.eq_bytes(b"exec_fd") {
        let raw0 = args.first().ok_or(BuiltinError::InvalidArgument)?;
        let varname = raw0
            .strip_prefix(b"%")
            .ok_or(BuiltinError::InvalidArgument)?;
        let fd = state
            .fds
            .get(&varname)
            .ok_or(BuiltinError::InvalidArgument)?;
        let exports: Vec<(sys::ShortCStr, Vec<u8>)> = state
            .exports
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        let args_slice = refs.get(1..).ok_or(BuiltinError::InvalidArgument)?;
        // exec-without-fork: the PID stays the same, so the shell must return
        // the child's exit code regardless of success or failure. Both outcomes
        // go into the Ok position — the Err arm only catches parse/syscall
        // errors from builtin-exec itself, not from exec_fd/exec_at.
        match exec::exec_fd(fd, args_slice, &exports) {
            Ok(()) => Ok(0),
            Err(report) => Ok(report.current_context().exit_code()),
        }
    } else if name.eq_bytes(b"exec_at") {
        let raw0 = args.first().ok_or(BuiltinError::InvalidArgument)?;
        let varname = raw0
            .strip_prefix(b"%")
            .ok_or(BuiltinError::InvalidArgument)?;
        let dirfd = state
            .fds
            .get(&varname)
            .ok_or(BuiltinError::InvalidArgument)?;
        let pathname = args.get(1).ok_or(BuiltinError::InvalidArgument)?;
        let pathname = sys::RefCStr::from(pathname.clone());
        // execveat with a relative pathname rejects dirfds that have FD_CLOEXEC set.
        // Create a non-CLOEXEC copy via export().
        let non_cloexec = dirfd.export().map_err(BuiltinError::from)?;
        let exports: Vec<(sys::ShortCStr, Vec<u8>)> = state
            .exports
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        let args_slice = refs.get(2..).ok_or(BuiltinError::InvalidArgument)?;
        // Same exec-without-fork semantics as exec_fd — always Ok(code).
        match exec::exec_at(non_cloexec.at(), &pathname, args_slice, &exports) {
            Ok(()) => Ok(0),
            Err(report) => Ok(report.current_context().exit_code()),
        }
    } else if name.eq_bytes(b"resolve") {
        let name = refs.first().ok_or(BuiltinError::InvalidArgument)?;
        let fd = match exec::resolve_path(name) {
            Ok(fd) => fd,
            Err(report) => return Ok(report.current_context().exit_code()),
        };
        sys::shellfd::send_fd(&fd, c"resolve").ok();
        Ok(0)
    } else {
        let name_bytes = name.as_bytes().unwrap_or(&[]);
        match crate::child::fdpass::dispatch(name_bytes, args, state) {
            Some(Ok(v)) => Ok(v),
            Some(Err(report)) => Ok(match report.current_context() {
                crate::error::fdpass::FdPassError::SendFailed => sys::errno::EIO,
                crate::error::fdpass::FdPassError::NotFound
                | crate::error::fdpass::FdPassError::InvalidName
                | crate::error::fdpass::FdPassError::MissingArg => sys::errno::EINVAL,
            }),
            None => Err(BuiltinError::Unknown),
        }
    }
}
