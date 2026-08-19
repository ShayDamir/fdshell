use crate::AppError;
use crate::cli::CliArgs;
use alloc::collections::VecDeque;
use alloc::vec::Vec;
use error_stack::{Report, ResultExt};
use sys::ImportedStr;
use sys::Origin;

type ScriptResult = Option<(Vec<u8>, VecDeque<ImportedStr>, Origin)>;

pub fn load_script_source(parsed: &CliArgs) -> Result<ScriptResult, Report<AppError>> {
    let positional: VecDeque<ImportedStr> = parsed.positional.iter().cloned().collect();

    if let Some(fd) = &parsed.script_fd {
        let origin = parsed.script_origin.clone().unwrap_or(Origin::Stdin);
        return Ok(Some((
            crate::cli::load_script(fd).change_context(AppError::ScriptRead)?,
            positional,
            origin,
        )));
    }

    if let Some(path) = parsed.positional.first() {
        let cstr = path.value.export();
        let fd = if let Some(dirfd) = &parsed.dirfd {
            sys::openat2::openat2(
                dirfd.at(),
                &cstr,
                &sys::openat2::OpenHow::new(sys::fcntl::O_RDONLY as u64, 0),
            )
            .change_context(AppError::ScriptRead)?
        } else {
            sys::openat2::open(&cstr, sys::fcntl::O_RDONLY).change_context(AppError::ScriptRead)?
        };
        return Ok(Some((
            crate::cli::load_script(&fd).change_context(AppError::ScriptRead)?,
            positional,
            Origin::File(path.value.clone()),
        )));
    }

    Ok(None)
}

#[cfg(test)]
mod tests;
