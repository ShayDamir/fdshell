#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
use super::*;
use alloc::vec;

/// A live child (exits 0) as a `Task` — the deque's entries own a real pidfd,
/// so a jobs test forks one and reaps it itself (hermetic: no child outlives
/// the test, LESSONS).
fn child_task() -> Task {
    let (pid, pidfd_opt) = sys::fork_pidfd::fork_pidfd().unwrap();
    match pidfd_opt {
        None => sys::exit(0),
        Some(pidfd) => Task {
            pidfd,
            capture_fd: None,
            child_pid: pid,
            captures: Vec::new(),
        },
    }
}

/// Reap every task of a drained/depleted table (the tests' teardown: each
/// removal hands back a live child, and every one of them is waited here, so
/// the test leaves no zombie behind).
fn reap(tasks: impl IntoIterator<Item = Task>) {
    for task in tasks {
        let _ = task.pidfd.wait_pidfd_rusage();
    }
}

#[test]
fn push_anon_appends_in_fifo_order() {
    let mut jobs = Jobs::new();
    assert!(jobs.is_empty());
    let first = child_task();
    let second = child_task();
    let (first_pid, second_pid) = (first.child_pid, second.child_pid);
    jobs.push_anon(first);
    jobs.push_anon(second);
    assert_eq!(jobs.anon_len(), 2);
    assert_eq!(jobs.anon_pids(), vec![first_pid, second_pid]);
    assert!(!jobs.is_empty());
    reap(jobs.drain_anon());
    assert!(jobs.is_empty());
}

#[test]
fn remove_by_pid_rotates_the_deque_and_pops_the_job() {
    let mut jobs = Jobs::new();
    let (a, b, c) = (child_task(), child_task(), child_task());
    let (pa, pb, pc) = (a.child_pid, b.child_pid, c.child_pid);
    jobs.push_anon(a);
    jobs.push_anon(b);
    jobs.push_anon(c);
    // The tail job: the two before it rotate to the tail, keeping FIFO order.
    let got = jobs.remove_by_pid(pc).unwrap();
    assert_eq!(got.child_pid, pc);
    assert_eq!(jobs.anon_pids(), vec![pa, pb]);
    reap([got]);
    // The front job: no rotation needed, the deque order is unchanged.
    let got = jobs.remove_by_pid(pa).unwrap();
    assert_eq!(got.child_pid, pa);
    assert_eq!(jobs.anon_pids(), vec![pb]);
    reap([got]);
    let got = jobs.remove_by_pid(pb).unwrap();
    assert_eq!(got.child_pid, pb);
    assert_eq!(jobs.anon_len(), 0);
    reap([got]);
}

#[test]
fn remove_by_pid_prefers_the_named_table() {
    let mut jobs = Jobs::new();
    let task = child_task();
    let pid = task.child_pid;
    let name = ShortCStr::from_vec(b"named".to_vec()).unwrap();
    jobs.insert_named(name.clone(), task);
    let got = jobs.remove_by_pid(pid).unwrap();
    assert_eq!(got.child_pid, pid);
    assert_eq!(jobs.named_len(), 0, "the named entry was the handle");
    assert_eq!(jobs.anon_len(), 0);
    reap([got]);
    assert!(jobs.remove_by_pid(pid).is_none());
}

#[test]
fn named_keys_lists_the_named_table() {
    let mut jobs = Jobs::new();
    let task = child_task();
    let name = ShortCStr::from_vec(b"k1".to_vec()).unwrap();
    jobs.insert_named(name.clone(), task);
    assert_eq!(jobs.named_keys(), vec![name.clone()]);
    assert!(jobs.contains_named(&name));
    reap(jobs.drain_named());
    assert!(!jobs.contains_named(&name));
}

#[test]
fn anon_deque_is_untouched_by_removal_of_a_name() {
    // `unset`/`waitpid &name` only ever touch the named table: a nameless job
    // survives every named removal (`unset` of a name that never held one).
    let mut jobs = Jobs::new();
    let task = child_task();
    let pid = task.child_pid;
    jobs.push_anon(task);
    assert!(
        jobs.remove_named(&ShortCStr::from_vec(b"gone".to_vec()).unwrap())
            .is_none()
    );
    assert_eq!(jobs.anon_len(), 1);
    assert_eq!(jobs.anon_pids(), vec![pid]);
    reap(jobs.drain_anon());
}
