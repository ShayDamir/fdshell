use alloc::vec::Vec;
use core::ffi::CStr;
use error_stack::{Report, bail};

use super::Capture;
use super::commit::{CapturedFd, CapturedValue};
use crate::error::capture::CaptureError;
use crate::state::{FdArrayEntry, FdVar, ShellState};
use sys::{LocalFd, Origin, ShortCStr, Trace};

/// A capture waiting for fds: how many more it may still take.
pub(super) struct Slot {
    pub(super) cap: Capture,
    pub(super) entries: Vec<FdArrayEntry>,
    pub(super) room: usize,
}

impl Slot {
    pub(super) fn new(c: Capture, state: &ShellState) -> Result<Slot, Report<CaptureError>> {
        if !c.force && state.fds.contains_key(&c.var) {
            bail!(CaptureError::Exists);
        }
        let taken = state.arrays.get(&c.var).map_or(0, |a| a.len());
        let room = match c.cap {
            None => 1,
            Some(n) => n.saturating_sub(taken),
        };
        Ok(Slot {
            cap: c,
            entries: Vec::new(),
            room,
        })
    }

    pub(super) fn needs_more(&self) -> bool {
        self.entries.len() < self.room
    }

    pub(super) fn matches(&self, rtag: &CStr) -> bool {
        match self.cap.tag.as_ref() {
            Some(t) => t.eq_bytes(rtag.to_bytes()),
            None => true,
        }
    }

    pub(super) fn take(&mut self, fd: LocalFd, rtag: &CStr) {
        let source = tag_name(rtag);
        self.entries.push(FdArrayEntry {
            fd,
            source: source.clone(),
            trace: Trace::at(self.cap.set_at, Origin::Captured(source)),
        });
    }

    pub(super) fn satisfied(&self) -> bool {
        self.cap.cap.is_some() || !self.entries.is_empty()
    }

    pub(super) fn finish(self) -> Result<CapturedFd, Report<CaptureError>> {
        match self.cap.cap {
            None => {
                let entry = self.entries.into_iter().next().ok_or(CaptureError::Never)?;
                Ok(CapturedFd {
                    var: self.cap.var,
                    value: CapturedValue::Fd(FdVar {
                        fd: entry.fd,
                        trace: entry.trace,
                    }),
                })
            }
            Some(_) => Ok(CapturedFd {
                var: self.cap.var,
                value: CapturedValue::Array(self.entries),
            }),
        }
    }
}

/// A received SHELLFD tag is NUL-free up to its terminator, so `push` is infallible.
fn tag_name(rtag: &CStr) -> ShortCStr {
    let mut name = ShortCStr::new();
    name.push(rtag);
    name
}
