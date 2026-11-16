//! Per-function-call variable frames: the names shadowed by `local` and the
//! values to restore when the call returns. The shape is `run_env::EnvSave`
//! (LESSONS: scoped assignments save `strings`, `exports` and `IFS` together).

// mutants: -C --lib -- -E 'test(/^state::frames::/)'

pub(crate) mod cell;

use alloc::vec::Vec;
use error_stack::Report;
use hashbrown::HashMap;
use sys::{ImportedStr, ShortCStr};

use super::ShellState;
use crate::error::cmd::CmdError;

/// The first-touch `strings`/`exports` values of every name shadowed in one
/// call, plus the old `IFS` and the old `OPTIND` counter when a shadow touches
/// them: `set_var` syncs `state.ifs` and `state.optind`, and both are **fields**
/// — the per-name entry restore cannot reach them (the entry itself now holds
/// the startup seed, so it restores too). Rule: `restore()` writes the stores
/// directly and never goes through `set_var`, so **every** mirror field must be
/// carried by the frame, not just `IFS` (LESSONS: "A save/restore frame must
/// carry").
pub(crate) struct VarFrame {
    entries: HashMap<ShortCStr, (Option<ImportedStr>, Option<ImportedStr>)>,
    old_ifs: Option<ShortCStr>,
    old_optind: Option<u64>,
}

impl ShellState {
    /// Enter a function-call frame; `local` records its shadows here.
    pub fn begin_frame(&mut self) {
        self.frames.push(VarFrame {
            entries: HashMap::new(),
            old_ifs: None,
            old_optind: None,
        });
    }

    /// Leave the innermost frame, putting every shadowed value back and
    /// writing the saved mirror fields (`IFS`, the `OPTIND` counter) last.
    /// Without a frame this is a no-op.
    pub fn end_frame(&mut self) {
        let Some(frame) = self.frames.pop() else {
            return;
        };
        for (name, (old_strings, old_exports)) in frame.entries {
            restore(&mut self.strings, &name, old_strings);
            restore(&mut self.exports, &name, old_exports);
        }
        if let Some(ifs) = frame.old_ifs {
            self.ifs = ifs;
        }
        if let Some(optind) = frame.old_optind {
            self.optind = optind;
        }
    }

    /// Record `name`'s current values as the restore target. First touch only,
    /// so `local x=1; local x=2` restores the caller's value, not `1`. The
    /// mirror fields ride along on the same first touch: `IFS` saves `old_ifs`,
    /// `OPTIND` saves `old_optind`.
    pub fn shadow(&mut self, name: &ShortCStr) -> Result<(), Report<CmdError>> {
        let frame = self.frames.last_mut().ok_or(CmdError::Never)?;
        if frame.entries.contains_key(name) {
            return Ok(());
        }
        let old_strings = self.strings.get(name).cloned();
        let old_exports = self.exports.get(name).cloned();
        if name.eq_bytes(b"IFS") {
            frame.old_ifs = Some(self.ifs.clone());
        }
        if name.eq_bytes(b"OPTIND") {
            frame.old_optind = Some(self.optind);
        }
        frame
            .entries
            .insert(name.clone(), (old_strings, old_exports));
        Ok(())
    }

    /// True inside a function call: `local` is legal only there.
    pub fn in_frame(&self) -> bool {
        !self.frames.is_empty()
    }

    /// The innermost frame's shadowed names, unsorted (the `local` list form
    /// sorts them with `intercept::set_list::sort_by_bytes`).
    pub fn local_names(&self) -> Vec<&ShortCStr> {
        self.frames
            .last()
            .map(|frame| frame.entries.keys().collect())
            .unwrap_or_default()
    }
}

/// Put one `name`'s saved value back in a store: `Some` writes it, `None`
/// removes the key (the declare form `local NAME` leaves the name unset).
fn restore(
    store: &mut HashMap<ShortCStr, ImportedStr>,
    name: &ShortCStr,
    saved: Option<ImportedStr>,
) {
    match saved {
        Some(value) => {
            store.insert(name.clone(), value);
        }
        None => {
            let _ = store.remove(name);
        }
    }
}

#[cfg(test)]
mod tests;
