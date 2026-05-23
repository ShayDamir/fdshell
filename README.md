# FD Shell - small, security-oriented shell with file descriptor passing.

FD shell aims to reduce number of security vulnerabilities in common shell scripts
by allowing passing file descriptors between subprocesses in a controlled, safe way.

Currently, no external programs can return file descriptors, so shell scripts use builtin
implementations of common file descriptor operations.

Example:

```shell
# this creates new directory and saves the file descriptor to the directory in %foo
builtin mkdirat %CWD foo %>%foo
# this creates another directory and saves the file descriptor in %bar
builtin mkdirat %CWD bar %>%bar
# this creates new file in %foo
builtin openat %foo baz --creat --excl --mode %>%baz
# this renames the foo/baz into bar/qux
builtin renameat %foo baz %bar qux
# this spawn external command (/bin/echo) that writes to the same file descriptor that was created as foo/baz
echo "test" >%baz
```

By passing file descriptors directly instead of using paths to resolve them, scripts
written in fdshell can avoid TOCTOU vulnerabilities when parts of paths are changed
parallel to script invocation.

## How it works?

### Passing file descriptors from subprocess back to fdshell

Traditionally shell is responsible to set up file descriptors 0 (stdin), 1 (stdout)
and 2 (stderr) for launched subprocesses.

fdshell adds another file descriptor number 3 - shellfd. It is an anonymous UNIX
socket created using socketpair() syscall. The subprocess can use that file descriptor
to send its own file descriptors using SCM_RIGHTS mechanism. Along the file descriptor,
the subprocess also transfers a tag (string), which can be used by the fdshell to distinguish
the returned file descriptors.

For example, the `pipe` command creates two file descriptors called `rd`
(read side of the pipe) and `wr` (write side of the pipe). These could be saved into
different variables using this syntax:

```shell
# creates anynymous pipe file descriptors
builtin pipe %rd>%server %wr>%client
# sends request to the pipe
echo "request" >%client
unset %client # closes the client fd
# receives request from the pipe
REQUEST=$(cat <%server) # REQUEST="request"
```

If the received file descriptors are not assigned to a variable, they're immediately closed.

### Passing file descriptors to subprocess

File descriptor variables can be passed in several ways:

* as stdin redirection (`<%var`) or stdout redirection (`>%var`)
* as a specified file descriptor number (`2>%var` or `5<%var`)
* as an command line argument `%var`

In first 2 cases, fdshell will use `dup2()` call to replace the specified file descriptor
between `fork()` and `exec()`.

In the latter case, the file descriptor number will determined by the result of `dup()` syscall
between `fork()` and `exec()`, and fdshell will substitute `%var` with the resulting number.

For example, if `dup()` returned `63`, the command line argument `--fd=%var` will be substitued
as `--fd=63`. The launched subprocess then can use this number as a valid file descriptor.

### Addressing background tasks

When a subprocess is spawned in the background via `&`, fdshell will save the corresponding `pidfd`
of the subprocess in a special variable `%!`.

Example:

```shell
# launches server as background process
run_server params &
# saves its pidfd into %server_pid
%server_pid=%!
# sends signal to the background process
builtint kill --pidfd=%server_pid
# waits until server is finished
wait %server_pid
```

This avoids race conditions that are possible with traditional '$!' variable, which contains PID of the
latest spawned subprocess. When sending signal to the background process by PID, it is possible that
the process is already finished and PID was reused, and the signal will go to some unrelated process.

With `pidfd`, this type of race conditions can be avoided.

### Security concerns

The file descriptors are received from the spawned subprocesses using `MSG_CMSG_CLOEXEC` flag passed
to `recvmsg` syscall. This atomically sets the `CLOEXEC` bit, preventing the subprocesses from accessing
file descriptors stored in fdshell.

When file descriptors are passed to subprocesses as a fd redirection or as command line arguments,
`dup` or `dup2` syscalls are called after `fork`, but before `exec`. This strips the `CLOEXEC` flag
and allows the subprocess to access the passed file descriptors.

TODO: how to protect against leaking of fd 3 to the children of the subprocess? The subprocess should
immediately set CLOEXEC on fd 3?


### Implementation philosophy

The fdshell binary is a static binary that has only one dependency: OS kernel.

Currently only Linux is supported and only x86_64.

The project is a workspace, divided into 2 paths:

* safe - all crates there have forbid(unsafe). This also means that they cannot call libc directly.
* unsafe - lib crates that allow unsafe, but each file should not have more than 80 lines of code (not counting comments)

Currently following crates are planned;

safe/fdshell - implementation of main shell logic, including spawning and receiving file descriptors
safe/builtins - implementation of various builtin commands
unsafe/sys - syscalls and other wrappers needed to implement crates in safe

The ecosystem should avoid derive directives as much as possible to keep the code at minimum.
If possible, everything should be no_std
