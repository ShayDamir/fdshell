#![allow(clippy::unwrap_used)]

use super::*;
use alloc::vec;

fn identity(dev: u64, ino: u64) -> Identity {
    Identity { dev, ino }
}

fn state() -> ShellState {
    ShellState::new()
}

/// A stored buffer comes back on the next read of the same key — the round trip
/// the drain-first reader depends on — and the entry is **taken**, so a buffer is
/// served once.
#[test]
fn store_then_take_returns_the_bytes_once() {
    let mut state = state();
    state.store_read_pending(
        ReadKey::Fd(c"3".into()),
        || Some(identity(7, 40)),
        vec![b'b', b'\n'],
    );

    let live = identity(7, 40);
    assert_eq!(
        state.take_read_pending(&ReadKey::Fd(c"3".into()), || Some(live)),
        vec![b'b', b'\n']
    );
    // Taken: the second read of the same key starts from an empty buffer.
    assert!(
        state
            .take_read_pending(&ReadKey::Fd(c"3".into()), || Some(live))
            .is_empty()
    );
    assert!(state.read_pending.is_empty());
}

/// The fd number and the fd var name are separate namespaces, so a buffer stored
/// under one is not handed to the other (the spelling-aliasing decision of
/// task #609: fdshell clones an fd var per call, so it keys by name).
#[test]
fn the_two_namespaces_do_not_share_buffers() {
    let mut state = state();
    state.store_read_pending(
        ReadKey::Var(c"f".into()),
        || Some(identity(7, 40)),
        vec![b'x'],
    );

    let live = identity(7, 40);
    assert!(
        state
            .take_read_pending(&ReadKey::Fd(c"3".into()), || Some(live))
            .is_empty(),
        "the fd-number key does not see the var-name buffer"
    );
    assert_eq!(
        state.take_read_pending(&ReadKey::Var(c"f".into()), || Some(live)),
        vec![b'x']
    );
}

/// An empty tail stores nothing: the map holds only live buffers, and the identity
/// is not asked for when there is nothing to record (the unbuffered path stays
/// syscall-free).
#[test]
fn storing_an_empty_tail_stores_nothing() {
    let mut state = state();
    let mut asked = false;
    state.store_read_pending(ReadKey::Fd(c"3".into()), || ask(&mut asked), Vec::new());

    assert!(!asked, "an empty tail never asks for the identity");
    assert!(state.read_pending.is_empty());
}

/// A descriptor whose identity changed since the bytes were stored has lost them:
/// the entry is dropped and the read starts from an empty buffer — the rule that
/// keeps `exec <`, `exec 3<`, a `dup2`, a scope restore or a close/reopen of a raw
/// fd from turning the buffer into a new bug.
#[test]
fn a_changed_identity_drops_the_entry() {
    let mut state = state();
    state.store_read_pending(
        ReadKey::Fd(c"3".into()),
        || Some(identity(7, 40)),
        vec![b'b'],
    );

    let reopened = identity(7, 61);
    assert!(
        state
            .take_read_pending(&ReadKey::Fd(c"3".into()), || Some(reopened))
            .is_empty(),
        "bytes from the old descriptor are not this source's"
    );
    assert!(
        state.read_pending.is_empty(),
        "the stale entry is gone, not merely skipped"
    );
}

/// A failed identity query (a descriptor closed under us) also drops the entry.
#[test]
fn an_unanswerable_identity_drops_the_entry() {
    let mut state = state();
    state.store_read_pending(
        ReadKey::Fd(c"3".into()),
        || Some(identity(7, 40)),
        vec![b'b'],
    );
    assert!(
        state
            .take_read_pending(&ReadKey::Fd(c"3".into()), || None)
            .is_empty()
    );
    assert!(state.read_pending.is_empty());
}

/// The answer of a lazy identity probe: it records that it was asked.
fn ask(asked: &mut bool) -> Option<Identity> {
    *asked = true;
    None
}
