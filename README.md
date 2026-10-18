# FD Shell — small, security-oriented shell with file descriptor passing

FD shell aims to reduce the number of security vulnerabilities in common shell
scripts by allowing file descriptors to be passed between subprocesses in a
controlled, safe way.

A file descriptor is the small integer a Unix process uses to name an open
file, pipe, socket or directory. A shell normally manages only fd 0 (stdin),
1 (stdout) and 2 (stderr); fdshell lets a script create, keep, pass and
receive any number of descriptors.

That matters because a path is not a stable handle. An attacker can replace a
path component between the moment a script checks the path and the moment a
program opens it (a time-of-check-to-time-of-use race). An already-open
descriptor cannot be replaced: it is a handle to the file itself.

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

By passing file descriptors directly instead of using paths to resolve them,
scripts written in fdshell can avoid TOCTOU vulnerabilities when parts of
paths are changed in parallel to script invocation.

Currently no external program can return file descriptors by itself, so shell
scripts use the builtin implementations of the common file descriptor
operations.

## File descriptor variables

- `%name` is an **fd variable**: it holds a file descriptor, and expands to
  its number. `unset %name` closes the descriptor.
- `cmd %>%name` runs `cmd` and stores the descriptor `cmd` sent back under
  `%name`. A capture on a foreground command waits for it.
- `cmd %>%arr[N]` stores received descriptors in an fd array of at most `N`
  entries (a tagged capture `%tag>%name` only accepts descriptors sent with
  that tag), and `for %x in %arr; do …` iterates the array.
- `%CWD` is the directory the shell started in, kept open; `cd` re-opens it.
  Once a directory is an fd, operations can be pinned to it (`--dirfd %foo`),
  so renaming an ancestor does not redirect them.
- `%?` is the descriptor that became ready, inside a `wait` arm.
- A `%name` that is not bound stays literal text (`%nosuchfd` is
  `%nosuchfd`). A literal percent sign is written `\%`.

### Passing file descriptors to a subprocess

A descriptor can be handed to a child in the usual ways:

- as a standard descriptor: `<%var`, `>%var`, `2>%var`, `5<%var`;
- as a command-line argument: `prog --fd=%var`. The `%var` word becomes the
  number of a descriptor dup'ed for the child (if `dup()` returned `63`, the
  child sees `--fd=63`).

Redirections are applied with `dup2()` between `fork()` and `exec()`;
arguments are substituted with the number returned by `dup()` in the same
window.

### Receiving file descriptors from a subprocess

fdshell creates an anonymous UNIX socket for each child, passed through the
`FDSHELL_SOCKET` environment variable. A child that knows the protocol can
send its own descriptors over that socket with `SCM_RIGHTS`, each with a tag
string; fdshell matches the tag against the capture word and stores the
descriptor. Received descriptors not assigned to a variable are closed
immediately.

```shell
# creates anonymous pipe file descriptors
builtin pipe %rd>%server %wr>%client
# sends request to the pipe
echo "request" >%client
unset %client # closes the client fd
# receives request from the pipe
REQUEST=$(cat <%server) # REQUEST="request"
```

### Background tasks

A subprocess can be launched in the background and tracked under a name:

```shell
# launches server as a named background task
run_server params &>&server
# waits until server is finished and receives its captures
waitpid &server
```

The shell stores the task (a pidfd plus its capture context) in the task
variable `&server`; `&>|&server` overwrites an existing task. Because a pidfd
stays valid and unique for the lifetime of the child, this avoids the
PID-reuse races of the traditional `$!` / `kill` pattern.

The POSIX `wait [pid…]` builtin reaps background tasks by pid (or every task
when given no argument) and sets `$?` to the last reaped status; `$!` holds
the last background pid:

```shell
builtin false &>&j
wait $!            # reap by pid; $? is the child's exit status
wait               # reap every background task
```

### Event-case `wait`

`wait` also runs one poll round over the shell's fd variables. Every ready
descriptor runs its arm in a forked child; the arm's exit status decides the
descriptor's fate — exit `0` keeps it open for the next round, any other
status closes it. The ready descriptor is bound to `%?`. `after N` arms fire
when nothing is ready within `N` milliseconds.

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

Wrap the block in a loop (`while true; do wait … done`) to re-poll each round.

## Builtins

Builtins live in the shell process, not on `PATH`: invoke them as
`builtin openat2 …`. (`shopt -s builtin_first` makes a bare name resolve to
its builtin before searching `PATH`.) `help` lists every builtin, the shell
commands and the socket family.

| Command | Description |
|---|---|
| `openat2 [--dirfd N] [--mode MODE] [--resolve FLAGS] [--flags FLAGS] [--path] path` | Open or create a file via `openat2`. Returns one fd. |
| `mkdirat [--dirfd N] [--mode MODE] [--resolve FLAGS] path` | Create a directory via `mkdirat` + `openat2`. Returns one fd. |
| `mkfifoat [--dirfd N] [--mode MODE] [--resolve FLAGS] path` | Create a fifo via `mkfifoat` + `openat2 O_RDWR`. Returns one fd tagged `fifo`. |
| `pipe [--flags FLAGS]` | Create an anonymous pipe via `pipe2`. Returns two fds tagged `rd` and `wr`. |
| `renameat2 [--olddirfd N] [--newdirfd N] [--flags FLAGS] oldpath newpath` | Rename or exchange files via `renameat2`. Returns no fd. |
| `unlinkat [--dirfd N] [--flags AT_REMOVEDIR] path` | Remove a file, or a directory with `AT_REMOVEDIR`, via `unlinkat`. Returns no fd. |
| `symlinkat [--dirfd N] target linkpath` | Create a symbolic link via `symlinkat`. Returns no fd. |
| `utimensat [--dirfd N] [--atime SPEC] [--mtime SPEC] [--flags AT_SYMLINK_NOFOLLOW] path` | Set a file's atime/mtime via `utimensat`. Returns no fd. |

Flags are named constants (`O_CREAT`, `O_NONBLOCK`, `RENAME_NOREPLACE`, etc.)
or `0x`-prefixed hex values; repeat `--flags` to combine several.

`--mode` defaults to `0777` for `mkdirat` and to `0666` for `mkfifoat` and
`openat2` with `O_CREAT`; the kernel masks it with the process umask, so under
`umask 022` a mode-less `mkdirat` yields a `0755` directory. An explicit
`--mode` overrides the default.

`unlinkat --dirfd N` and `symlinkat --dirfd N` pin the *parent* directory to
the fd, but the kernel still resolves the final path component: keep the
trailing name short when it matters. `openat2 --path` opens an `O_PATH`
handle — no read/write permission on the file itself — which can be combined
with `statx %fd` to inspect files the user cannot open. `openat2 --same-as
%fd` checks the opened file against the `%fd` reference at open time, so on a
mismatch the capture is never committed:

```shell
builtin openat2 --flags O_RDONLY file %>%ref
builtin openat2 --same-as %ref --flags O_RDONLY file %>%f   # fails if file changed
```

## Redirection

A redirection sends one of a command's descriptors somewhere else:

| Operator | Meaning |
|---|---|
| `>` | write stdout (fd 1) |
| `>>` | append stdout |
| `<` | read stdin (fd 0) |
| `<>` | open a file for both reading and writing |
| `>|` | write, even when `noclobber` is on |
| `&>` | write stdout **and** stderr (fd 1 and fd 2) |
| `&>>` | append stdout and stderr |

A number before the operator selects the descriptor: `2> err` sends stderr
to `err`; without a prefix, `>`/`>>`/`>|`/`&>`/`&>>` mean fd 1 and `<`/`<>`
mean fd 0.

```shell
echo hello > out.txt          # write a file
echo more >> out.txt          # append to it
cat < out.txt                 # read it back
sort out.txt 2> errors.txt    # stderr to a file
cmd &> log.txt                # both output streams to one file
```

The target of `>` can also be `%var` (an fd variable), `/dev/fd/N` (a copy of
the open fd `N`), `>&1` (a copy of fd 1), `2>&%f` (a copy of the fd `%f`
refers to) or `2>&-` (close fd 2):

```shell
builtin openat2 --dirfd %CWD --flags O_CREAT --flags O_WRONLY --mode 0644 log %>%log
echo "written through the variable" >%log
```

`set -o noclobber` (`set -C`) refuses a `>` that would overwrite an existing
file; `>|` is the explicit override. Several redirections may be written on
one command; they are applied in order, so the last one on a given descriptor
wins.

A redirection belongs to the command it is written on and is undone when the
command finishes — the shell's own descriptors come back. `exec >file` has no
command to run, so its redirection stays for the rest of the script.

### Here-documents

A here-document feeds a command's stdin from the lines that follow it, up to
a delimiter line:

```shell
cat <<EOF
line one
line two
EOF
```

An unquoted delimiter expands `$`/`$(…)`/backticks in the body; a quoted
delimiter (`<<'EOF'`) makes the body literal. `<<-EOF` strips leading tabs
from the body and the delimiter. A here-document is usually attached to
`cat`, `read` or another command that reads stdin.

A number in front of the operator puts the body on that descriptor instead of
stdin: `cat 3<<EOF` makes the body fd 3's input and leaves stdin alone, and
`exec 3<<EOF` keeps it there for the rest of the script. The number must be
its own unquoted digit word, as in `3>file` — `cat "3"<<EOF` and `cat \3<<EOF`
are the argument `"3"<<EOF`, not a here-document
([POSIX_COMPATIBILITY.md](POSIX_COMPATIBILITY.md#here-document-descriptor-numbers)).
`N` is a raw descriptor number, not an fd variable: it has no `%fd`
counterpart — nothing is bound under a name, `unset %name` closes nothing —
and the descriptor is reachable only as a number in the same command's
redirections (`3<<EOF <&3`, `exec 4>&3`).

A here-string feeds one expanded word plus a newline:

```shell
read -u 0 LINE <<<"hello $name"
```

## The shell language

fdshell follows the POSIX shell language, with the extensions listed above
and in [POSIX_COMPATIBILITY.md](POSIX_COMPATIBILITY.md).

### Words and quoting

A command is a sequence of words separated by spaces (and tabs). `'…'` is a
fully literal single-quoted span, `"…"` is a double-quoted span in which `$`,
backticks and `\` keep working, and `\x` outside quotes removes the special
meaning of `x`:

```shell
echo 'a   b'      # a   b        (spaces are data)
echo "hi $name"   # expands the variable
echo a\ b         # a b          (one word)
```

### Variables

`name=value` sets a shell variable; `$name` or `${name}` reads it:

```shell
greeting=hello
echo "$greeting, world"
export greeting        # visible to child processes
unset greeting         # remove
```

An unset variable expands to the empty string; `set -u` makes that an error
instead. `local name=value` (inside a function) scopes a variable to the
current call. `read name` reads a line from stdin into a variable.

### Parameter expansion

| Form | Result |
|---|---|
| `${name}` | the value (empty when unset) |
| `${name:-word}` | `word` when `name` is unset or empty |
| `${name:=word}` | assign `word` to `name` and print it |
| `${name:+word}` | `word` when `name` is set and non-empty |
| `${name:?word}` | fail with `word` as the message |
| `${name#pat}` / `${name##pat}` | remove the shortest/longest matching prefix |
| `${name%pat}` / `${name%%pat}` | remove the shortest/longest matching suffix |
| `${#name}` | length of the value |
| `${!name}` | value of the variable whose name is `name`'s value |

The positional and special parameters are also available: `$1`, `$2`, … are
the arguments, `$#` the argument count, `$0` the program name, `$?` the last
status, `$$` the shell pid, `$!` the last background pid. `"$@"` expands to
the arguments as separate words; `"$*"` joins them with the first `IFS` byte.

### Command substitution

`$(cmd)` and `` `cmd` `` run `cmd` and substitute its output:

```shell
files=$(ls)
echo "there are $(echo "$files" | wc -l) entries"
```

### Word splitting and pathname expansion

The result of an expansion is split into words on the `IFS` bytes (space, tab
and newline by default), and then each word containing `*`, `?` or `[...]` is
expanded against the filesystem:

```shell
IFS=:; parts=$PATH        # split PATH into a : separated list
for f in *.txt; do …; done
```

A quoted byte never splits and never globs. `set -f` disables pathname
expansion; `shopt -s dotglob` lets patterns match a leading dot;
`shopt -s nullglob` deletes a pattern with no match; `shopt -s failglob`
makes it an error.

### Brace expansion

`{a,b,c}` produces one word per alternative, and `{1..5}` a range:

```shell
echo {foo,bar}.txt        # foo.txt bar.txt
echo img{01..03}.png      # img01.png img02.png img03.png
```

Quoted braces are literal.

### Arithmetic

`$((expr))` evaluates a C-style integer expression and substitutes the
result; `((expr))` evaluates it and sets the exit status; `let` evaluates its
arguments:

```shell
echo $((2 + 3 * 4))       # 14
i=0
while ((i < 10)); do i=$((i + 1)); done
let x=3+4
```

The language has the usual precedence, `&&`/`||`, comparisons, the ternary
`?:`, `++`/`--`, assignments and the comma operator, plus decimal, hex
(`0xff`), octal (`010`) and `base#number` literals.

### Conditionals and loops

```shell
if test -f "$file"; then
    echo "found"
elif [ "$file" = "-" ]; then
    echo "stdin"
else
    echo "missing"
fi

for name in a b c; do echo "$name"; done
for ((i = 0; i < 3; i++)); do :; done
while read line; do echo "$line"; done
until [ -e ready ]; do sleep 1; done

case "$1" in
    start) echo starting ;;
    stop|halt) echo stopping ;;
    *) echo "unknown" ;;
esac
```

`break` and `continue` accept an optional loop count (`break 2`). The status
of a command is `0` for success and non-zero for failure, so `&&` runs the
next command only on success and `||` only on failure:

```shell
make && echo built || echo "build failed"
```

### Functions and groups

```shell
greet() {
    local who=$1
    echo "hello, $who"
}
greet world

{ echo a; echo b; } > both.txt     # `{ list; }` groups commands in the current shell
```

A function sees its arguments as `$1..`; a group shares the surrounding
shell, so its assignments and `cd` survive it.

### Pipelines and lists

`cmd1 | cmd2` connects cmd1's stdout to cmd2's stdin. `;` separates
statements, `&&`/`||` gate them, and `&` is the background operator (fdshell
also offers named background tasks, above). `time cmd` reports how long cmd
took; `timeout seconds cmd` stops it after a deadline.

### Aliases

```shell
alias ll='ls -l'
unalias ll
```

## Strict mode

The `strict` shell option (off by default) turns fdshell into a capability
shell in the Capsicum spirit: the `*at` file builtins ban absolute path
resolution and require every operation to be relative to an **explicit
`--dirfd`**:

```shell
shopt -s strict
builtin mkdirat --dirfd %CWD --mode 0755 sub %>%sub
builtin openat2 --dirfd %sub --flags O_CREAT --flags O_EXCL --flags O_RDWR --mode 0644 f %>%f
shopt -u strict
```

While `strict` is on, `openat2`, `mkdirat`, `mkfifoat`, `renameat2`,
`unlinkat`, `symlinkat` and `utimensat` reject an omitted `--dirfd` (or
`--dirfd AT_FDCWD`) and an absolute path. Redirect targets and program words
have no per-command `--dirfd`, so strict resolves them through the shell's
pinned `%CWD` descriptor and bans filesystem-root paths: a redirect target, a
program word containing `/`, a `hash` pin, and every absolute `PATH` entry
must stay relative. `exec`/`become` inherit the ban. `cd ..` remains the
user's own escape hatch: the base follows the shell, exactly as a relative
path does.

## Sockets

`bind`, `listen`, `accept` and `connect` give a script both the server and
the client side of an AF_UNIX or AF_INET v4 socket. `accept` blocks until a
connection arrives, then returns it as an fd variable:

```shell
builtin listen @srv %>%l
builtin accept %l %>%conns[8]     # up to 8 connections
```

`@name` is the abstract socket namespace (no filesystem object); a path is a
filesystem socket the script must `unlinkat` after use; `--addr ADDR --port N`
is AF_INET v4. Backgrounding an `accept` (`builtin accept %l %>%c &>&t;
waitpid &t`) or using it in a `wait readable` arm keeps the script responsive.

`sendmsg` and `recvmsg` move a byte payload together with any number of fd
variables in one SCM_RIGHTS message and are the primitive for custom AF_UNIX
protocols:

```shell
sendmsg %sock [--msg TEXT | --msgfd %var COUNT] [--fd %var]... [--passcred]
recvmsg [--cred VAR] %sock VAR [%fdvar ...]
```

With `--passcred` (sender) and `--cred CRED` (receiver), `recvmsg` also
reports the peer's identity as `PID:UID:GID`. On EOF (the peer closed),
`recvmsg` leaves `VAR` empty and exits 1.

## Security

Descriptors received from subprocesses are accepted with the
`MSG_CMSG_CLOEXEC` flag, which atomically sets the close-on-exec bit, so a
child cannot reach into the shell's descriptors. When fdshell passes a
descriptor to a child, `dup`/`dup2` run between `fork` and `exec` and strip
that bit only for the descriptors the child is meant to have. The shell's
capture socket is reserved close-on-exec at startup, and nested shells
re-apply the flag to the inherited socket, so it never leaks past an `exec`.

Every forked child (pipeline stages, launched commands, `$(…)` bodies, `wait`
arms) runs with `SIGPIPE` at its default disposition, so a pipeline stage
dies silently when its reader goes away (`seq 1000000 | echo got` prints
`got` and nothing else), while the interactive shell itself survives a closed
stdout.

## Implementation

The fdshell binary is a dynamically linked Linux x86_64 binary with no
dependencies beyond the kernel APIs it calls directly via syscalls. Currently
only Linux x86_64 is supported.

The project is a workspace with three crates:

* `safe/fdshell` — main shell logic, spawning and receiving file descriptors
* `safe/builtins` — builtin commands (no_std, `forbid(unsafe)`)
* `unsafe/sys` — syscall wrappers (no_std, unsafe allowed)

The safe crates cannot call libc directly; all raw file descriptors live in
`unsafe/sys`, behind `LocalFd`/`ImportedFd`/`ExportedFd` types that encode
ownership and close-on-exec rules.

## Compatibility

fdshell implements the POSIX shell language and the POSIX utilities it
provides; see [POSIX_COMPATIBILITY.md](POSIX_COMPATIBILITY.md) for the few
deliberate, permanent deviations.
