//! The **shell string arrays** (task #422): the store behind
//! [`ShellState::str_arrays`], its writers, and the one-namespace rule bash
//! gives every name. The `%` fd arrays of [`super::arrays`] are a different
//! namespace with a different store; the two never cross (task #422's scope
//! decision, README "Variables").
//!
//! Values are `ImportedStr` like `strings` ones, so the element's origin keeps
//! the trace bookkeeping the plain assignments have. The store is sparse and
//! sorted by index: bash answers `a=(); a[5]=x` with `${a[@]}` = `x` and
//! `${#a[@]}` = `1`.

use alloc::vec;
use alloc::vec::Vec;

use sys::ImportedStr;
use sys::ShortCStr;

use super::ShellState;

impl ShellState {
    /// The one-namespace rule: the store's elements in index order, plus the
    /// **scalar at index 0** when the store holds no index 0. That single rule
    /// is the whole scalar/array bridge — `a=hello; echo "${a[0]}"` → `hello`,
    /// `a=hello; echo "${#a[@]}"` → `1`, `a=hello; a+=(x); echo "${a[@]}"` →
    /// `hello x`, `a[1]=x; echo "${a[@]}"` → `x`. Values, not `ImportedStr`s:
    /// the reads of task #422 own no tracing of their own.
    pub(crate) fn elements_of(&self, name: &ShortCStr) -> Vec<ShortCStr> {
        let Some(store) = self.str_arrays.get(name) else {
            return self
                .scalar_value(name)
                .map(|v| vec![v.clone()])
                .unwrap_or_default();
        };
        let mut out = Vec::new();
        if !store.contains_key(&0)
            && let Some(scalar) = self.scalar_value(name)
        {
            out.push(scalar.clone());
        }
        for element in store.values() {
            out.push(element.value.clone());
        }
        out
    }

    /// `name=(w1 w2 …)`: the store takes the name, one element per expanded
    /// word, indices sequential from 0 (bash measures them in that order).
    pub(crate) fn set_str_array(&mut self, name: &ShortCStr, elements: Vec<ImportedStr>) {
        let store = self.str_arrays.entry(name.clone()).or_default();
        store.clear();
        for (index, element) in elements.into_iter().enumerate() {
            store.insert(index, element);
        }
    }

    /// `name+=(…)`: the append starts at the next free index, so an append to a
    /// scalar-named variable lands at 1 and leaves the bridge's index 0 to the
    /// scalar (bash's `a=hello; a+=(x)` → `hello x`).
    pub(crate) fn append_str_array(&mut self, name: &ShortCStr, elements: Vec<ImportedStr>) {
        let next = self.next_str_index(name);
        let store = self.str_arrays.entry(name.clone()).or_default();
        for (offset, element) in elements.into_iter().enumerate() {
            store.insert(next.saturating_add(offset), element);
        }
    }

    /// `name[idx]=value`: one element write; `parse/array_ref.rs::split_index_ref`
    /// owns the index syntax.
    pub(crate) fn set_str_element(&mut self, name: &ShortCStr, index: usize, value: ImportedStr) {
        self.str_arrays
            .entry(name.clone())
            .or_default()
            .insert(index, value);
    }

    /// The index an append starts at: one past the store's highest element,
    /// 1 when the name holds only a scalar, 0 when it holds nothing.
    fn next_str_index(&self, name: &ShortCStr) -> usize {
        match self
            .str_arrays
            .get(name)
            .and_then(|store| store.keys().next_back())
        {
            Some(last) => last.saturating_add(1),
            None if self.scalar_value(name).is_some() => 1,
            None => 0,
        }
    }

    /// One element **of the store**, no bridge: `None` is the caller's
    /// out-of-range reading, decided against `set -u` in `substitute`.
    pub(crate) fn str_element(&self, name: &ShortCStr, index: usize) -> Option<&ShortCStr> {
        self.str_arrays
            .get(name)
            .and_then(|store| store.get(&index))
            .map(|element| &element.value)
    }

    /// True when the store holds `name` as an array — what `set -u` demands of
    /// `${#name[@]}`, which bash refuses for a mere scalar (`a: unbound
    /// variable`, measured, task #422).
    pub(crate) fn holds_str_array(&self, name: &ShortCStr) -> bool {
        self.str_arrays.contains_key(name)
    }

    /// `unset name` takes the array store's entry too — bash takes the name out
    /// of every store at once (see [`super::setters`]).
    pub(crate) fn remove_str_array(&mut self, name: &ShortCStr) {
        let _ = self.str_arrays.remove(name);
    }
}

#[cfg(test)]
mod tests;
