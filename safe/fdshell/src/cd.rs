use error_stack::{Report, ResultExt};

use crate::error::cd::CdError;
use crate::state::{FdVar, ShellState};
use sys::fcntl::{O_DIRECTORY, O_NOFOLLOW};
use sys::{LocalFd, Origin, Position, ShortCStr, Trace};

/// The directory to move to. The caller (`intercept/cd.rs`) decides **which**
/// form a `cd` line is: it owns the raw word, the expanded fields and the
/// usage error, while `cd` only opens the target it is handed.
#[derive(Debug)]
pub(crate) enum CdTarget {
    /// `$HOME`: the bare `cd`, and every operand whose expansion produced no
    /// field at all (`cd "$@"` with an empty positional deque).
    Home,
    /// `OLDPWD`: the `cd -` form, which bash reads on the **expanded** field
    /// (`D=-; cd "$D"` is the old cwd even when a directory named `-` exists).
    OldCwd,
    /// The fdshell-only `%name` form, read on the **raw** word: `name` carries
    /// **no** `%`, the fd table's key shape. Expanding the word would hand the
    /// fd *number* to the path arm and drop the stored `Origin`.
    Var(ShortCStr),
    /// A path, opened relative to the shell's cwd.
    Path(ShortCStr),
}

pub(crate) fn cd(
    target: CdTarget,
    state: &mut ShellState,
    set_at: Position,
) -> Result<(), Report<CdError>> {
    let (new_fd, origin) = match target {
        CdTarget::Home => cd_home()?,
        CdTarget::OldCwd => cd_var(&c"OLDCWD".into(), state)?,
        CdTarget::Var(name) => cd_var(&name, state)?,
        CdTarget::Path(path) => cd_path(&path)?,
    };
    new_fd.fchdir().change_context(CdError::CdPathOpen)?;
    state.move_cwd(new_fd, origin, set_at);
    Ok(())
}

fn cd_home() -> Result<(LocalFd, Origin), Report<CdError>> {
    let home = sys::env::getenv(c"HOME").ok_or(CdError::HomeNotSet)?;
    let fd = open_dir(&home)?;
    Ok((fd, Origin::EnvVar(ShortCStr::from(c"HOME"))))
}

/// Open the fd variable `%name`. The fd table is keyed by the **bare** name
/// (the `%` is the word's syntax, `substitute/percent.rs` looks the same name
/// up), so `name` arrives without it — the caller strips the prefix off the
/// raw word ([`CdTarget::Var`]) and the `%` is never added back.
fn cd_var(name: &ShortCStr, state: &ShellState) -> Result<(LocalFd, Origin), Report<CdError>> {
    let src = state.fds.get(name).ok_or(CdError::FdNotSet)?;
    let fd = src.fd.try_clone().change_context(CdError::CdPathOpen)?;
    Ok((fd, src.trace.origin.clone()))
}

fn cd_path(path: &ShortCStr) -> Result<(LocalFd, Origin), Report<CdError>> {
    let fd = open_dir(path)?;
    Ok((fd, Origin::File(path.clone())))
}

fn open_dir(path: &ShortCStr) -> Result<LocalFd, Report<CdError>> {
    // `+` not `|`: disjoint flags, `|` would leave an unkillable `|`-`^` mutant (LESSONS.md)
    sys::openat2::open(path.export(), O_DIRECTORY + O_NOFOLLOW).change_context(CdError::CdPathOpen)
}

impl ShellState {
    fn move_cwd(&mut self, new_cwd: LocalFd, origin: Origin, set_at: Position) {
        let cwd_key: ShortCStr = c"CWD".into();
        if let Some(old) = self.fds.remove(&cwd_key) {
            self.fds.insert(c"OLDCWD".into(), old);
        }
        self.fds.insert(
            cwd_key,
            FdVar {
                fd: new_cwd,
                trace: Trace::at(set_at, origin),
            },
        );
    }
}

#[cfg(test)]
mod tests;
