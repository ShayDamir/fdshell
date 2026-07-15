use crate::AppError;
use crate::cli::CliArgs;
use alloc::collections::VecDeque;
use alloc::vec::Vec;
use error_stack::{Report, ResultExt};
use sys::ShortCStr;

type ScriptResult = Option<(Vec<u8>, VecDeque<ShortCStr>)>;

pub fn load_script_source(parsed: &CliArgs) -> Result<ScriptResult, Report<AppError>> {
    if let Some(fd) = &parsed.script_fd {
        let pos: VecDeque<ShortCStr> = parsed
            .positional
            .iter()
            .map(|a| ShortCStr::from_vec(a.to_bytes().to_vec()).unwrap_or_default())
            .collect();
        return Ok(Some((
            crate::cli::load_script(fd).change_context(AppError::ScriptRead)?,
            pos,
        )));
    }

    if let Some(path) = parsed.positional.first() {
        let fd = if let Some(dirfd) = &parsed.dirfd {
            sys::openat2::openat2(
                dirfd.at(),
                path.as_c_str(),
                &sys::openat2::OpenHow::new(
                    (sys::fcntl::O_RDONLY | sys::fcntl::O_CLOEXEC) as u64,
                    0,
                ),
            )
            .change_context(AppError::ScriptRead)?
        } else {
            sys::openat2::open(path.as_c_str(), sys::fcntl::O_RDONLY)
                .change_context(AppError::ScriptRead)?
        };
        let mut pos = VecDeque::new();
        pos.push_back(ShortCStr::from_vec(path.to_bytes().to_vec()).unwrap_or_default());
        pos.extend(
            parsed
                .positional
                .iter()
                .skip(1)
                .map(|a| ShortCStr::from_vec(a.to_bytes().to_vec()).unwrap_or_default()),
        );
        return Ok(Some((
            crate::cli::load_script(&fd).change_context(AppError::ScriptRead)?,
            pos,
        )));
    }

    Ok(None)
}
