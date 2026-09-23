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

## Heredocs

A here-doc feeds a command's stdin from the script body: the lines after the
command line, up to (not including) the first line that byte-exactly equals the
delimiter. The body is delivered through an anonymous memfd.

```shell
cat <<EOF
line one
line two
EOF
```

Forms: `cmd <<DELIM` (attached), `cmd << DELIM` (separate word), and
`cmd <<"DELIM"` (quoted). An unquoted delimiter runs `$` / `$(…)` / backtick
expansion in the body; a quoted delimiter makes the body literal.
`<<""` (empty quoted delimiter) reads a body up to the next empty line.
The body is otherwise opaque: `;`, `&&`, `|`, `#`, `$(…)`, quotes, and
keyword-shaped lines (`fi`, `done`, `}`) are all body content. An empty body
is zero bytes, and no trailing newline is appended — the body keeps the last
line's own newline (`ab\n` is 3 bytes). A here-doc works in pipelines
(`cat <<EOF | wc -l`) and in block bodies (if/while/for/case/function), like
any other command.

Limitations:

- `<<-DELIM` (tab-stripping form) is not supported.
- A here-doc in a block condition or cond-list position (`if cat <<EOF; then`,
  `cat <<EOF && x`) is rejected with `here-doc: missing terminating delimiter
  line`.
- The REPL reads line-based input, so a here-doc at the prompt must be entered
  as one block.
- Here-docs inside `$( )` / backticks, an `N<<EOF` fd prefix (like `N<<<`
  today), and a `# comment` after the operator on the same line are not
  supported.
- A body line that is exactly `elif` / `else` (in an if body) or `;;` (in a
  case / wait body) is misread as block structure.
- A second stdin redirect on one command (two here-docs, or a here-doc plus
  `< file`) is rejected with `duplicate redirect target`.

## Glob expansion

A word containing an unquoted `*`, `?`, or a valid `[...]` bracket expression
is expanded against the filesystem (bash pathname-expansion semantics):

- `*` matches any run of bytes except `/`; `?` matches exactly one byte;
  `[...]` matches one byte from a set — with `a-z` ranges, `!` negation,
  and a leading `]` as a literal member.
- Matching is per path component: `*` never crosses a `/`. Unquoted `/`
  separates pattern components, which are walked literally (`echo sub/*`).
  A leading `/` makes the pattern absolute; a trailing `/` matches
  directories only and is kept in the results (`echo */` → `sub/`).
- `.` and `..` are never matched by a pattern component, and a pattern
  cannot consume a name's leading `.` unless the pattern starts with a
  literal `.` (`echo *` skips `.hidden`; `echo .h*` matches it).
- Symlinks to directories are followed at intermediate positions
  (`ln -s sub link; echo link/*` works).
- Results are sorted bytewise. A pattern with no matches is passed through
  verbatim (bash default); `shopt -s nullglob` makes it disappear.

Quoting: double-quoted bytes always match literally; a fully quoted word is
never expanded. Expansion happens for command arguments (external, builtins,
`builtin`, `become`), `set --` words, user-function arguments, unquoted
`$@` / `$*` fields (which re-glob, like bash), and literal `for … in` words.
`$(…)` / `$((…))` outputs, `case` words, redirect targets, and command names
are not expanded (v1).

Limitations:

- No `[:class:]` character classes inside brackets.
- No `dotglob` / `failglob` (bash defaults are off; shopt follow-up planned).
- Redirect target words and command names (word 0) are not globbed.
- Backslash: fdshell keeps `\X` in the word (argv semantics — `echo \*`
  passes `\*` to the program), so an escaped pattern character does not
  trigger expansion: `echo a\*` prints `a\*` verbatim where bash prints
  `a*`.

## How it works?

### Passing file descriptors from subprocess back to fdshell

Traditionally shell is responsible to set up file descriptors 0 (stdin), 1 (stdout)
and 2 (stderr) for launched subprocesses.

fdshell adds an anonymous UNIX socket called shellfd, created via `socketpair()`.
The subprocess number is communicated through the `FDSHELL_SOCKET` environment variable.
The subprocess can use that file descriptor to send its own file descriptors using
SCM_RIGHTS mechanism. Along the file descriptor, the subprocess also transfers a tag
(string), which can be used by the fdshell to distinguish the returned file descriptors.

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
a background task (pidfd + capture context) in a pidvar `name`. The `waitpid` builtin reaps the
child and processes any pending captures.

```shell
# launches server as a named background task
run_server params &>&server
# waits until server is finished and receives its captures
waitpid &server
```

Multiple background tasks can be created and waited on independently:

```shell
build &>&builder
test &>&tester
waitpid &builder
waitpid &tester
```

The `&>|&name` variant (with `|`) forces an overwrite if a task with that name already exists.

Foreground subprocess with captures is equivalent to background + immediate wait:

```shell
cmd %>output                    # foreground — wait + captures synchronous
cmd %>output &>&x; waitpid &x   # same, via explicit background + wait
```

This avoids race conditions possible with traditional `$!` / PID-based background tracking,
since pidfds remain valid and unique for the lifetime of the tracked child.

### Event-case `wait`

`wait` runs one poll round over fd variables. For every descriptor that is ready it runs
the matching arm in a forked child; the arm's exit status decides the descriptor's fate —
exit `0` keeps it open for the next round, any other status closes it (the `finished` arm
closes unconditionally). The ready descriptor is bound to `%?`. `after N` arms fire when
nothing is ready within `N` milliseconds.

```shell
# wait for data on the pipe's read end, or for a background job to finish
builtin pipe %>%rd %>%wr
echo "ping" >%wr
echo hi &>&job
wait
    readable %rd)
        read -u %? LINE
        echo "got $LINE" ;;
    finished %&job)
        echo "job $?" ;;
    after 1000)
        echo "no data" ;;
done
echo "wait exited $?"
```

Wrap it in a loop (`while true; do wait … done`) to re-poll each round. Arms run in
separate children, so a slow arm (a blocking read) cannot stall the others or the parent.

### Security concerns

The file descriptors are received from the spawned subprocesses using `MSG_CMSG_CLOEXEC` flag passed
to `recvmsg` syscall. This atomically sets the `CLOEXEC` bit, preventing the subprocesses from accessing
file descriptors stored in fdshell.

When file descriptors are passed to subprocesses as a fd redirection or as command line arguments,
`dup` or `dup2` syscalls are called after `fork`, but before `exec`. This strips the `CLOEXEC` flag
and allows the subprocess to access the passed file descriptors.

The shellfd is reserved with CLOEXEC on startup, so it is automatically closed at exec boundaries. In nested shells, `try_into_local()` sets CLOEXEC on the inherited capture socket to prevent it from leaking through subsequent execs.


### Implementation philosophy

The fdshell binary is a dynamically linked Linux x86_64 binary (glibc + libgcc_s). It has no dependencies beyond the OS kernel APIs it calls directly via syscalls.

Currently only Linux is supported and only x86_64.

The project is a workspace with three crates:

* `safe/fdshell` - main shell logic, spawning and receiving file descriptors
* `safe/builtins` - builtin commands (no_std, forbid(unsafe))
* `unsafe/sys` - syscall wrappers (no_std, unsafe allowed)

All `safe/` crates have `forbid(unsafe_code)` and cannot call libc directly. Non-test source files should aim for ≤90 lines, though a few exceed this limit (e.g. `debug.rs`, `caret.rs`, `error/parse.rs`).

The codebase uses `#[derive]` where idiomatic — commonly `Clone`, `Default`, `Debug`, `PartialEq`, and `Display`. Error types derive `Display` + `Debug`. The shell crate (`safe/fdshell`) is `#![no_std]` with `extern crate std` for runtime stability.
