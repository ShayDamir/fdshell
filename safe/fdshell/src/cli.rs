use error_stack::{Report, ResultExt};
use std::ffi::CString;

use crate::AppError;

/// Read script content from a LocalFd into a Vec<u8>.
pub fn load_script(fd: &sys::LocalFd) -> Result<Vec<u8>, Report<AppError>> {
    let mut content = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        let n = fd.read(&mut buf).change_context(AppError::ScriptRead)?;
        if n == 0 {
            break;
        }
        let n = n as usize;
        let slice = buf.get(..n).ok_or(AppError::ScriptRead)?;
        content.extend_from_slice(slice);
    }
    Ok(content)
}

/// Parsed result of CLI argument parsing.
pub struct CliArgs {
    pub dirfd: Option<sys::ImportedFd>,
    pub script_fd: Option<sys::LocalFd>,
    pub positional: Vec<CString>,
}

/// Parse `--dirfd`, `--fd`, and positional arguments.
///
/// `all_args` is already the `skip(1)` slice (i.e. does not include the binary name).
pub fn parse_cli_args(all_args: &[CString]) -> Result<CliArgs, Report<AppError>> {
    let mut dirfd: Option<sys::ImportedFd> = None;
    let mut script_fd: Option<sys::LocalFd> = None;
    let mut positional: Vec<CString> = Vec::new();

    let mut i = 0;
    while i < all_args.len() {
        let arg = all_args.get(i).ok_or(AppError::Usage)?;
        match arg.to_bytes() {
            b"--dirfd" => {
                i += 1;
                let num_str = all_args.get(i).ok_or(AppError::MissingValue("dirfd"))?;
                dirfd = Some(
                    sys::ImportedFd::try_from(num_str.as_ref())
                        .change_context(AppError::InvalidFdNumber("dirfd"))?,
                );
            }
            b"--fd" => {
                i += 1;
                let num_str = all_args.get(i).ok_or(AppError::MissingValue("fd"))?;
                let imported = sys::ImportedFd::try_from(num_str.as_ref())
                    .change_context(AppError::InvalidFdNumber("fd"))?;
                script_fd = Some(
                    imported
                        .try_into_local()
                        .change_context(AppError::CloexecFailed)?,
                );
            }
            _ => {
                positional.push(arg.clone());
            }
        }
        i += 1;
    }

    Ok(CliArgs {
        dirfd,
        script_fd,
        positional,
    })
}
