use crate::task::Task;
use crate::vars::FdVars;
use std::collections::HashMap;
use sys::ShortCStr;
use sys::errno::EINVAL;
use sys::siginfo::WaitStatus;

pub(crate) fn run_one(
    line: &str,
    fdvars: &mut FdVars,
    tasks: &mut HashMap<ShortCStr, Task>,
    last_status: &mut WaitStatus,
) -> Result<(), i32> {
    match crate::parse::parse(line)? {
        crate::parse::ParsedLine::Cmd(cmdline) => {
            if crate::intercept::try_intercept(&cmdline, fdvars, tasks, last_status)? {
                return Ok(());
            }
            let outcome = crate::launch::launch(fdvars, &cmdline)?;
            *last_status = crate::postlaunch::finish_cmd(cmdline, outcome, fdvars, tasks)?;
        }
        crate::parse::ParsedLine::Pipeline(pipeline) => {
            *last_status = crate::postlaunch::run_pipeline(pipeline, fdvars)?;
        }
        crate::parse::ParsedLine::Assign { var, value } => {
            let src = fdvars.resolve(&value).ok_or(EINVAL)?;
            fdvars.insert(var, src.try_clone()?);
            *last_status = WaitStatus::Exited(0);
        }
        crate::parse::ParsedLine::Unset(var) => {
            fdvars.remove(&var);
            tasks.remove(&var);
            *last_status = WaitStatus::Exited(0);
        }
        crate::parse::ParsedLine::Umask(mask) => {
            if let Some(m) = mask {
                sys::umask::set(m);
            } else {
                println!("{:04o}", sys::umask::get());
            }
            *last_status = WaitStatus::Exited(0);
        }
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use sys::siginfo::WaitStatus;

    fn child_test(f: impl FnOnce()) {
        let (_, pidfd_opt) = sys::fork_pidfd::fork_pidfd().unwrap();
        match pidfd_opt {
            None => {
                sys::umask::init();
                let saved = sys::umask::get();
                f();
                sys::umask::set(saved);
                std::process::exit(42);
            }
            Some(pidfd) => {
                let status = sys::wait_pidfd::wait_pidfd(&pidfd).unwrap();
                match status {
                    WaitStatus::Exited(42) => {}
                    other => panic!("unexpected status {}", other.exit_code()),
                }
            }
        }
    }

    #[test]
    fn umask_set_via_run_one() {
        child_test(|| {
            let mut fdvars = FdVars::new();
            let mut tasks = HashMap::new();
            let mut last_status = WaitStatus::Exited(0);
            run_one("umask 0o077", &mut fdvars, &mut tasks, &mut last_status).unwrap();
            assert!(matches!(last_status, WaitStatus::Exited(0)));
            assert_eq!(sys::umask::get(), 0o077);
        });
    }

    #[test]
    fn umask_set_zero_via_run_one() {
        child_test(|| {
            let mut fdvars = FdVars::new();
            let mut tasks = HashMap::new();
            let mut last_status = WaitStatus::Exited(0);
            run_one("umask 0o000", &mut fdvars, &mut tasks, &mut last_status).unwrap();
            assert!(matches!(last_status, WaitStatus::Exited(0)));
            assert_eq!(sys::umask::get(), 0o000);
        });
    }

    #[test]
    fn umask_set_without_o_prefix() {
        child_test(|| {
            let mut fdvars = FdVars::new();
            let mut tasks = HashMap::new();
            let mut last_status = WaitStatus::Exited(0);
            run_one("umask 077", &mut fdvars, &mut tasks, &mut last_status).unwrap();
            assert!(matches!(last_status, WaitStatus::Exited(0)));
            assert_eq!(sys::umask::get(), 0o077);
        });
    }

    #[test]
    fn umask_invalid_returns_err() {
        child_test(|| {
            let mut fdvars = FdVars::new();
            let mut tasks = HashMap::new();
            let mut last_status = WaitStatus::Exited(0);
            let e = run_one("umask abc", &mut fdvars, &mut tasks, &mut last_status).unwrap_err();
            assert_eq!(e, EINVAL);
        });
    }

    #[test]
    fn umask_too_many_args_returns_err() {
        child_test(|| {
            let mut fdvars = FdVars::new();
            let mut tasks = HashMap::new();
            let mut last_status = WaitStatus::Exited(0);
            let e = run_one(
                "umask 0o077 extra",
                &mut fdvars,
                &mut tasks,
                &mut last_status,
            )
            .unwrap_err();
            assert_eq!(e, EINVAL);
        });
    }
}
