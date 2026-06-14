# FD Shell — small, security-oriented shell with file descriptor passing.

FD shell aims to reduce number of security vulnerabilities in common shell scripts
by allowing passing file descriptors between subprocesses in a controlled, safe way.

Currently, no external programs can return file descriptors, so shell scripts use builtin
implementations of common file descriptor operations.

Example:

```shell
# creates a directory and saves the fd in %foo
builtin mkdirat --dirfd %CWD --mode 0755 foo %>%foo
# creates another directory and saves the fd in %bar
builtin mkdirat --dirfd %CWD --mode 0755 bar %>%bar
# creates a new file in %foo
builtin openat2 --dirfd %foo --flags O_CREAT --flags O_EXCL --flags O_RDWR --mode 0644 baz %>%baz
# renames foo/baz into bar/qux
builtin renameat2 --olddirfd %foo --newdirfd %bar baz qux
# spawns an external command that writes to the same fd as foo/baz
echo "test" >%baz
```

By passing file descriptors directly instead of using paths to resolve them, scripts
written in fdshell can avoid TOCTOU vulnerabilities when parts of paths are changed
parallel to script invocation.

## Builtins

| Command | Description |
|---|---|
| `openat2 [--dirfd N] [--mode MODE] [--resolve FLAGS] [--flags FLAGS] path` | Open or create a file via `openat2`. Returns one fd. |
| `mkdirat [--dirfd N] [--mode MODE] [--resolve FLAGS] path` | Create a directory via `mkdirat` + `openat2`. Returns one fd. |
| `pipe [--flags FLAGS]` | Create an anonymous pipe via `pipe2`. Returns two fds tagged `rd` and `wr`. |
| `renameat2 [--olddirfd N] [--newdirfd N] [--flags FLAGS] oldpath newpath` | Rename or exchange files via `renameat2`. Returns no fd. |

Flags are named constants (`O_CREAT`, `O_NONBLOCK`, `RENAME_NOREPLACE`, etc.) or
`0x`-prefixed hex values. Repeat `--flags` to combine multiple flags.

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

Subprocesses can be launched in the background using the `&>&name` syntax. The shell stores
a background task (pidfd + capture context) in a pidvar `name`. The `wait` builtin reaps the
child and processes any pending captures.

```shell
# launches server as a named background task
run_server params &>&server
# waits until server is finished and receives its captures
wait &server
```

Multiple background tasks can be created and waited on independently:

```shell
build &>&builder
test &>&tester
wait &builder
wait &tester
```

The `&>|&name` variant (with `|`) forces an overwrite if a task with that name already exists.

Foreground subprocess with captures is equivalent to background + immediate wait:

```shell
cmd %>output            # foreground — wait + captures synchronous
cmd %>output &>&x; wait &x   # same, via explicit background + wait
```

This avoids race conditions possible with traditional `$!` / PID-based background tracking,
since pidfds remain valid and unique for the lifetime of the tracked child.

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
* unsafe - lib crates that allow unsafe, but each file should not have more than 80 lines of code (not counting comments).
  Temporarily suspended during large-scale refactoring (error-handling migration).

Currently following crates are planned;

safe/fdshell - implementation of main shell logic, including spawning and receiving file descriptors
safe/builtins - implementation of various builtin commands
unsafe/sys - syscalls and other wrappers needed to implement crates in safe

The ecosystem should avoid derive directives as much as possible to keep the code at minimum.
If possible, everything should be no_std
