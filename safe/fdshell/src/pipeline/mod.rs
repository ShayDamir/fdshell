#![forbid(unsafe_code)]

use crate::capture::Capture;
use crate::child::{self, Command};
use crate::parse::Pipeline;
use crate::redirect::{Redirect, RedirectDirection, RedirectSource};
use crate::vars::FdVars;
use sys::siginfo::WaitStatus;

pub struct CaptureChannel {
    pub capture_fd: sys::LocalFd,
    pub child_pid: i32,
    pub captures: Vec<Capture>,
}

pub fn launch_pipeline(
    vars: &FdVars,
    pipeline: Pipeline,
) -> Result<(WaitStatus, Vec<CaptureChannel>), i32> {
    let n = pipeline.commands.len();
    let commands = pipeline.commands;

    let mut pipes: Vec<(sys::LocalFd, sys::LocalFd)> = Vec::with_capacity(n.saturating_sub(1));
    for _ in 0..n.saturating_sub(1) {
        let (rd, wr) = sys::pipe::pipe2(sys::fcntl::O_CLOEXEC)?;
        pipes.push((rd, wr));
    }

    let mut capture_pairs: Vec<Option<(sys::LocalFd, sys::LocalFd)>> = Vec::with_capacity(n);
    for cmd in &commands {
        if cmd.captures.is_empty() {
            capture_pairs.push(None);
        } else {
            let (parent, child) = sys::net::socketpair()?;
            sys::net::set_passcred(&parent)?;
            capture_pairs.push(Some((parent, child)));
        }
    }

    let mut children: Vec<(i32, sys::LocalFd)> = Vec::with_capacity(n);

    for i in 0..n {
        let (child_pid, pidfd_opt) = sys::fork_pidfd::fork_pidfd()?;
        match pidfd_opt {
            None => {
                let cmd_data = match commands.get(i) {
                    Some(c) => c,
                    None => std::process::exit(sys::errno::EINVAL),
                };

                let mut redirects: Vec<Redirect<'_>> = Vec::new();

                if let Some(prev) = i.checked_sub(1).and_then(|p| pipes.get(p)) {
                    redirects.push(Redirect {
                        export_to: 0,
                        local: &prev.0,
                    });
                }
                if let Some(wr) = pipes.get(i) {
                    redirects.push(Redirect {
                        export_to: 1,
                        local: &wr.1,
                    });
                }

                let mut opened: Vec<sys::LocalFd> = Vec::with_capacity(cmd_data.redirects.len());
                for r in &cmd_data.redirects {
                    if let RedirectSource::Path(path) = &r.source {
                        let flags = match r.direction {
                            RedirectDirection::Read => sys::fcntl::O_RDONLY,
                            RedirectDirection::Write => {
                                sys::fcntl::O_WRONLY | sys::fcntl::O_CREAT | sys::fcntl::O_TRUNC
                            }
                        };
                        let name = path.to_c_string();
                        let fd = match sys::openat2::openat2(
                            sys::atfd::AtFd::cwd(),
                            &name,
                            &sys::openat2::OpenHow {
                                flags: flags as u64 | sys::fcntl::O_CLOEXEC as u64,
                                mode: 0o666,
                                resolve: 0,
                            },
                        ) {
                            Ok(f) => f,
                            Err(e) => std::process::exit(e),
                        };
                        opened.push(fd);
                    }
                }

                let mut path_idx = 0;
                for r in &cmd_data.redirects {
                    let local = match &r.source {
                        RedirectSource::Var(var) => match vars.resolve(var.as_bytes()) {
                            Some(fd) => fd,
                            None => std::process::exit(sys::errno::EINVAL),
                        },
                        RedirectSource::Path(_) => {
                            let fd = match opened.get(path_idx) {
                                Some(f) => f,
                                None => std::process::exit(sys::errno::EIO),
                            };
                            path_idx += 1;
                            fd
                        }
                    };
                    redirects.push(Redirect {
                        export_to: r.export_to,
                        local,
                    });
                }

                let child_sock = match capture_pairs.get_mut(i) {
                    Some(pair) => pair.take().map(|(_, ch)| ch),
                    None => None,
                };

                let cmd = if cmd_data.builtin {
                    Command::Builtin(cmd_data.command.clone())
                } else {
                    Command::External(cmd_data.command.clone())
                };

                child::child_exec(child_sock, vars, cmd, &cmd_data.args, &redirects)
            }
            Some(pidfd) => children.push((child_pid as i32, pidfd)),
        }
    }

    drop(pipes);

    let mut channels: Vec<CaptureChannel> = Vec::new();
    for i in 0..n {
        if let Some(pair) = capture_pairs.get_mut(i)
            && let Some((parent_end, child_end)) = pair.take()
        {
            drop(child_end);
            if let (Some(ch), Some(cmd)) = (children.get(i), commands.get(i)) {
                channels.push(CaptureChannel {
                    capture_fd: parent_end,
                    child_pid: ch.0,
                    captures: cmd.captures.clone(),
                });
            }
        }
    }
    drop(capture_pairs);
    drop(commands);

    let last = children.last().ok_or(sys::errno::EINVAL)?;
    let last_status = sys::wait_pidfd::wait_pidfd(&last.1)?;

    for i in 0..n.saturating_sub(1) {
        if let Some(ch) = children.get(i) {
            let _ = sys::wait_pidfd::wait_pidfd(&ch.1);
        }
    }

    Ok((last_status, channels))
}
