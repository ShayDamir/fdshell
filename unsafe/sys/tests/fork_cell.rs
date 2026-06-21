#![allow(clippy::unwrap_used)]

use sys::SyscallError;
use sys::fork_cell::ForkCell;
use sys::siginfo::WaitStatus;

// ---------------------------------------------------------------------------
// Basic cell operations work normally (no fork needed).
// ---------------------------------------------------------------------------

#[test]
fn cell_normal_borrow() {
    let cell = ForkCell::new(42);
    assert_eq!(*cell.borrow().unwrap(), 42);
}

#[test]
fn cell_normal_mut_borrow() {
    let cell = ForkCell::new(42);
    *cell.borrow_mut().unwrap() += 10;
    assert_eq!(*cell.borrow().unwrap(), 52);
}

// ---------------------------------------------------------------------------
// fork_pidfd_cell: the child resets the borrow counter and can borrow_mut.
// We signal success via a file descriptor — the child creates a named pipe,
// writes "ok" through it, and exits 0. The parent reads it back.
// For simplicity we just check the exit code.
// ---------------------------------------------------------------------------

/// Spawn a child that resets ForkCell and borrows mutably inside it.
/// Success: child exits 0 (and we see the expected state after borrow_mut).
#[test]
fn fork_pidfd_cell_child_mut_borrow() -> Result<(), SyscallError> {
    let cell = ForkCell::new(100);

    let (ret, pidfd_opt) = sys::fork_pidfd::fork_pidfd_cell(&cell)?;
    if ret == 0 {
        // Child: reset_after_fork should succeed, then borrow_mut.
        // SAFETY: we are in the forked child process -- exclusive ownership of
        // this copy of memory; calling reset_after_fork is safe.
        unsafe { cell.reset_after_fork() };
        *cell.borrow_mut().unwrap() += 1;
        std::process::exit(0);
    }

    // Parent's copy is unchanged by child mutation (fork gives separate address spaces)
    assert_eq!(*cell.borrow().unwrap(), 100);

    let pidfd = pidfd_opt.ok_or(SyscallError::Other(sys::errno::EINVAL))?;
    match sys::wait_pidfd::wait_pidfd(&pidfd)? {
        WaitStatus::Exited(0) => Ok(()),
        _ => Err(SyscallError::Other(sys::errno::EINVAL)),
    }
}

/// Spawn a child that verifies reset_after_fork allows exclusive access
/// even though the parent had active borrows before forking.
#[test]
fn fork_pidfd_cell_with_active_borrows() -> Result<(), SyscallError> {
    let cell = ForkCell::new(42);

    // Hold a shared borrow in the parent (this is just to prove the counter
    // was non-zero before forking; its value doesn't survive the fork).
    let _parent_borrow = cell.borrow().unwrap();

    let (ret, pidfd_opt) = sys::fork_pidfd::fork_pidfd_cell(&cell)?;
    if ret == 0 {
        // Child: without reset_after_fork(), borrow_mut would fail because
        // SAFETY: we are in the forked child process -- exclusive ownership of
        // this copy of memory; calling reset_after_fork is safe.
        // the inherited count > 0. With reset, it should succeed.
        unsafe { cell.reset_after_fork() };
        *cell.borrow_mut().unwrap() = 999;
        std::process::exit(0);
    }

    // Parent's value unchanged — child mutated its own copy
    assert_eq!(*cell.borrow().unwrap(), 42);

    let pidfd = pidfd_opt.ok_or(SyscallError::Other(sys::errno::EINVAL))?;
    match sys::wait_pidfd::wait_pidfd(&pidfd)? {
        WaitStatus::Exited(0) => Ok(()),
        _ => Err(SyscallError::Other(sys::errno::EINVAL)),
    }
}

/// Parent and child both borrow the cell normally — no reset needed in parent.
#[test]
fn fork_pidfd_cell_parent_uses_borrow() -> Result<(), SyscallError> {
    let cell = ForkCell::new(7);

    let (ret, pidfd_opt) = sys::fork_pidfd::fork_pidfd_cell(&cell)?;
    if ret == 0 {
        // SAFETY: we are in the forked child process -- exclusive ownership of
        // this copy of memory; calling reset_after_fork is safe.
        // Child: reset and mutate.
        unsafe { cell.reset_after_fork() };
        *cell.borrow_mut().unwrap() += 1;
        std::process::exit(42);
    }

    let pidfd = pidfd_opt.ok_or(SyscallError::Other(sys::errno::EINVAL))?;
    // Parent can still borrow normally (the child's copy is separate).
    assert_eq!(*cell.borrow().unwrap(), 7);
    match sys::wait_pidfd::wait_pidfd(&pidfd)? {
        WaitStatus::Exited(42) => Ok(()),
        _ => Err(SyscallError::Other(sys::errno::EINVAL)),
    }
}

/// fork_pidfd_cell should work with any Send type, not just i32.
#[test]
fn fork_pidfd_cell_with_struct() -> Result<(), SyscallError> {
    let cell = ForkCell::new(MyStruct { a: 1, b: "hello" });

    let (ret, pidfd_opt) = sys::fork_pidfd::fork_pidfd_cell(&cell)?;
    if ret == 0 {
        // SAFETY: we are in the forked child process -- exclusive ownership of
        // this copy of memory; calling reset_after_fork is safe.
        unsafe { cell.reset_after_fork() };
        // Mutate through borrow_mut
        let s = cell.borrow_mut().unwrap();
        assert_eq!(s.a, 1);
        std::process::exit(0);
    }

    let pidfd = pidfd_opt.ok_or(SyscallError::Other(sys::errno::EINVAL))?;
    match sys::wait_pidfd::wait_pidfd(&pidfd)? {
        WaitStatus::Exited(0) => Ok(()),
        _ => Err(SyscallError::Other(sys::errno::EINVAL)),
    }
}

struct MyStruct {
    a: i32,
    b: &'static str,
}
