use super::{Kind, PollEntry, ReleaseKey, events};
use crate::error::wait::WaitError;
use crate::parse::wait_block::FdRef;
use crate::state::ShellState;
use alloc::vec::Vec;
use error_stack::Report;

/// Expand a pattern's fd reference into one poll entry per concrete descriptor.
pub(super) fn resolve(
    ref_: &FdRef,
    arm: usize,
    kind: Kind,
    state: &ShellState,
) -> Result<Vec<PollEntry>, Report<WaitError>> {
    let finished = matches!(kind, Kind::Finished);
    let mut out = Vec::new();
    match ref_ {
        FdRef::Var(name) => {
            let fd = state
                .fds
                .get(name)
                .ok_or(WaitError::NoFd { name: name.clone() })?;
            let (e, m) = events(kind, false);
            out.push(PollEntry::new(
                fd.fd.as_raw(),
                e,
                m,
                arm,
                ReleaseKey::Var(name.clone()),
                finished,
            ));
        }
        FdRef::Array(name) => {
            let arr = state
                .arrays
                .get(name)
                .ok_or(WaitError::NotAnArray { name: name.clone() })?;
            let (e, m) = events(kind, false);
            for ent in arr.iter() {
                let key = ReleaseKey::Array {
                    arr: name.clone(),
                    source: ent.source.clone(),
                };
                out.push(PollEntry::new(ent.fd.as_raw(), e, m, arm, key, finished));
            }
        }
        FdRef::Task(name) => {
            let task = state
                .tasks
                .get(name)
                .ok_or(WaitError::TaskNotFound { name: name.clone() })?;
            let (e, m) = events(kind, true);
            out.push(PollEntry::new(
                task.pidfd.as_raw(),
                e,
                m,
                arm,
                ReleaseKey::Task(name.clone()),
                true,
            ));
        }
    }
    Ok(out)
}
