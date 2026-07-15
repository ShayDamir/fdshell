use crate::error::child_process::ChildProcessError;
use builtins::error::BuiltinError;
use core::fmt::Write;
use error_stack::{Report, bail};
use sys::ShortCStr;

pub(crate) fn handle_builtin_error(
    name: ShortCStr,
    report: Report<BuiltinError>,
) -> Result<i32, Report<ChildProcessError>> {
    match *report.current_context() {
        BuiltinError::Unknown => bail!(ChildProcessError::NotABuiltin(name)),
        BuiltinError::Help => Ok(0),
        BuiltinError::InvalidArgument(_) | BuiltinError::MissingArgument(_) => {
            let _ = writeln!(crate::io::Stderr, "{report:?}");
            Ok(1)
        }
        BuiltinError::Io => Err(report.change_context(ChildProcessError::BuiltinExecutionFailed)),
        BuiltinError::Syscall => {
            if let Some(e) = report.downcast_ref::<sys::SyscallError>() {
                Ok(e.errno())
            } else {
                Ok(1)
            }
        }
        BuiltinError::SendFdFailed => {
            let _ = writeln!(crate::io::Stderr, "{report:?}");
            Ok(1)
        }
    }
}
