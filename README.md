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
- Quoting is the escape at the **redirect-target** layer: the `%` right
  after the operator names the descriptor only while **unquoted**, so
  `> "%name"` (and `'%name'`, `>| "%name"`, `>> "%name"`, `&> "%name"`)
  opens the **file** named `%name`, as bash does. At the **word** layer
  quoting is *not* an escape: a bound `%name` expands inside quotes too
  (`echo "%name"` prints the number), and `\%` is the escape there —
  task #252 owns that decision
  ([POSIX_COMPATIBILITY.md](POSIX_COMPATIBILITY.md#is-a-file-descriptor-namespace)).

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
PID-reuse races of the traditional `$!` / `kill` pattern. Quoting the whole
word quotes the operator: `run_server '&>&server'` is a plain argument and
launches no task, as bash does.

The plain POSIX `&` operator backgrounds a statement with no name at all. The
job is the shell forked: the child runs the statement through the identical
foreground path (functions, builtins, `cd`, a pipeline as one job) and exits
with its status, while the parent records the child as a **nameless job** that
`$!` reports and `wait` reaps, and continues immediately with status 0:

```shell
sleep 3 &              # background job, the shell keeps going
echo hi & echo fg      # both run; the parent never waits for the job
sleep 3 &
wait                   # reap every nameless job
```

A nameless job's state changes die with the child, so `cd /tmp &`, `exit &`
and `break &` leave the parent untouched: the loop-control keywords (`break`,
`continue`, `return`) are jobs too — the child runs the keyword in its own copy
of the shell and the signal dies with it, so the parent's loop, function frame
or script runs on with status 0 (`for i in 1; do break & echo x; done` prints
`x`, as bash does). Bash parity there is stdout and status only: bash warns for
a top-level `break`/`continue`/`return`, fdshell prints its own report from the
child and nothing for a `return` that escaped. `&` is only the operator at a
word's start: `a&b` stays one word, `"&"` is data, `&&`, `&>file`, `N>&N` and
`|&` keep their meanings, and a `>` before it owns the byte (`cat > &f` is a
redirect). A `&` must terminate a command — `& echo b`, `echo a ; & b`,
`echo a & ; echo b` and `echo a & | cat` are parse errors that run **nothing**,
as in bash.

A **here-doc** belongs to the statement the `&` terminates: the body lines under
that line are its body, not statements of the shell. A second here-doc written
**behind** the `&` on the same line is a statement of its own, and each operator
takes the body it owns — the line's body regions are handed out in operator
order:

```shell
cat <<A & cat <<B    # the job runs `cat <<A`; `cat <<B` runs in the foreground
x                    # the background statement's body
A                    # its delimiter line
y                    # the foreground statement's body
B
wait                 # reap the job; a `wait` with lines below it opens the
                     # event-`wait` block, so write `wait;` or keep it last
```

A `&` written **inside a block's text** separates statements inside the block;
it does not end the block. The closer may sit right behind it on the same line,
and the statement in front of the `&` becomes a background job of the body, so
`$!` and `wait` see it:

```shell
for i in 1 2; do echo hi & done    # one job per iteration, `done` closes the loop
{ echo hi & }                      # the group's body statement runs as a job
case x in x) echo a & ;; esac      # `;;` ends the arm, `esac` ends the block
if true & then echo x; fi          # a `&`-terminated condition runs as a job
f(){ echo hi & }                   # the function body keeps its `&`
```

`for`'s word list stays strict: a **standalone** operator word anywhere in it is
the operator, not a word — the `&` (task #360), `&&`, `||`, the pipeline `|` and a
`;` that is not the list's own terminator before `do` (task #524) — so
`for i in 1 & do :; done`, `for i in 1 & ; do :; done`, `for i in 1 && ; do :;
done`, `for i in 1 | ; do :; done` (`&;`, `;&` and `; & ;` spell the same list)
and the newline spelling
(`for i in 1 &` newline `do :; done`) are parse errors — bash stops at rc 2,
fdshell at rc 1 with `expected 'do'`. A quoted `"&"` / `'&'` / `"&&"`, an escaped
`\&` and a glued `x&` stay word data, and the parens stay words for now (task
#470 owns them). A `&` behind a `&`, or a `;;` behind a `&`
outside a block (`echo a & ;;
echo b`), is the parse error that runs nothing. A `&`-terminated **condition**
is a background job too, one per condition run, so a loop records one per
iteration (`while : & do …; break; done`): the arm it gates sees the async
status 0 (`if true & then echo x; fi` prints `x`, and so does
`if false & then echo x; else echo y; fi`), the condition's side effects stay
in the discarded child (`if cd /tmp & then :; fi` leaves the cwd alone), and the
`&`-newline form is the same shape (`if true &` newline `then echo x; fi`). An
AND-list condition is the job too: `if true && false & then echo x; fi` prints
`x` like bash. What stays divergent: a condition whose last part is
short-circuited away (`if false && true & then echo x; fi`) records no job and
gates on that part's status, and a **compound** condition (`{ … } &`,
`for … done &`) runs foreground — bash backgrounds the whole list/compound
(owner #352). The fully **empty** condition of this family (`if & then`,
`while & do`) is a parse error since task #299 — there is no condition to
background (see "Conditionals and loops").

The POSIX `wait [pid…]` builtin reaps background tasks by pid (or every task
when given no argument) and sets `$?` to the last reaped status; `$!` holds
the last background pid:

```shell
builtin false &>&j
wait $!            # reap by pid; $? is the child's exit status
wait               # reap every background task (POSIX: status 0)
echo hi &
wait $!            # reap a nameless job by pid; $? is its exit status
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

`umask`, `unset` and the loop-control words `break`/`continue`/`return` are
**parse-level shell commands, not builtins**: the `builtin`/`command` prefix does
not reach them (`builtin umask` → `"umask" is not a shell builtin`,
`command umask` → `"umask" not found`, rc 1), and a quoted spelling is
transparent — `'umask' 022`, `'unset' %f`, `'break'` behave exactly like the
unquoted words. See `POSIX_COMPATIBILITY.md`, "Quoted keywords are command words,
quoted control keywords are not".

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
| `&>` | write stdout **and** stderr (fd 1 and fd 2), opening the file once and sharing it |
| `&>>` | append stdout and stderr (one open, shared by fd 1 and fd 2) |

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

`&>` / `&>>` open their operand **once** and put the same open file description
on fd 1 and fd 2, as bash's `> f 2>&1` does, so the two streams append to each
other (`echo hi; echo e >&2` writes `hi\ne\n`). Writing the same file with two
operators — `cmd > f 2> f` — is two separate opens, and the second one
truncates at offset 0.

A command's redirections apply **in the order they are written**, and each one
sees the descriptors the earlier ones left: `cmd > f 2>&1` puts fd 2 into `f`
(the `2>&1` copies the fd 1 the earlier redirection installed), while
`cmd 2>&1 > f` leaves fd 2 on the terminal. The target opens follow the same
order and stop at the first failure, so the side effects of the earlier opens
survive a later failure (`cmd > f 2> missing/dir/x` creates the empty `f`,
`cmd 2> missing/dir/x > f` creates nothing) — as bash applies them. A pipeline
stage's own pipe ends are part of that state before the stage's redirections
run, so `cat missing 2>&1 | cat > g` puts the message into the pipe, and
`cat missing > f 2>&1 | cat > g` puts it in `f`.

The target of `>` can also be `%var` (an fd variable), `/dev/fd/N` (the file
that open fd `N` points at — the magic path is **opened** with the operator's
flags as the redirection runs, so `> /dev/fd/1` truncates the file stdout is on
and `cat < /dev/fd/0` reads it from byte 0; only the canonical spelling counts,
`/dev/fd/01` is an ordinary path), `>&1` (a copy of fd 1), `2>&%f` (a copy of
the fd `%f` refers to) or `2>&-` (close fd 2):

```shell
builtin openat2 --dirfd %CWD --flags O_CREAT --flags O_WRONLY --mode 0644 log %>%log
echo "written through the variable" >%log
```

An operand that begins with `<`, `>`, `&` or `%` is a file name when that
byte is quoted — `echo hi > "&f"` opens `&f`, as bash does — and a syntax
error when it is not (`> &f`); the spaced `%var` operand stays a syntax
error, the fd-variable targets being the attached `>%var` / `&>%var` forms
([POSIX_COMPATIBILITY.md](POSIX_COMPATIBILITY.md#operators-are-recognized-at-word-starts)).
The **empty** operand stays a syntax error as well (`> ""`, `> ''`, `&> ""`):
bash parses it and fails at the open (`bash: : No such file`, rc 1), and
fdshell's rc 1 is the parse — the rc agrees, the message differs. A quoted
empty pair is a delimiter only for the here-document family
([Here-documents](#here-documents)).
The attached fd-variable target obeys the same quoting rule on its **own `%`
byte**: the `%` names a descriptor only while unquoted, so `> "%log"`,
`> '%log'`, `>| "%log"`, `>> "%log"` and `&> "%log"` create/append the
**file** `%log` as bash does, while `>&%var` — a dup, never a file operand
in bash — keeps its fd lookup quoted and unquoted alike.

The target of a `>`-family operator is a **word**: `$( … )`, `` ` … ` ``,
`$(( … ))`, `$name`, `${name…}`, `$@`/`$*` and a leading `~` expand in it, the
result is split on `IFS`, and each field is expanded against the filesystem,
relative to the last `builtin cd`.

```shell
echo hi > $(echo f)      # writes the file `f`
echo hi > "$(echo f)"    # the same: `"` never suppresses `$( … )`
echo hi > '$(echo f)'    # the file named `$(echo f)`: `'` does suppress
d=sub; echo hi > $d/x    # opens `sub/x`
echo hi > a1*            # the one match of `a1*`
```

The target must expand to **exactly one word**. A target that expands to
nothing (`> $(true)`, `> $@` with no positionals) or to several
(`n="a b"; echo hi > $n`, a glob with two matches) is an `ambiguous redirect`
(rc 1) and the command does not run. An unmatched pattern keeps its literal
word — `> zzz*` creates the file `zzz*`, which is also what `set -f` gives —
and the `shopt -s nullglob`/`failglob` behaviour on a redirect word is the
accepted divergence (task #369). A quoted byte never splits and never globs,
and never suppresses a substitution region: only `'` does.

The fd target forms are decided at parse time — the attached `>%var` and the
`>&N` duplicate word — so a `%` byte inside a target word is file-name data:
`> a%b` creates `a%b`. A `/dev/fd/N` target is an ordinary word at parse time
and only becomes a magic path at open time (task #337), which is why a target
that merely *expands* to the magic form keeps the pre-command descriptor (the
accepted divergence, task #479). The `>&N` duplicate word is not a word to expand
(`>&$(echo 1)` reads the `1` literally, task #370). In strict mode the ban on
an absolute target is checked on the **expanded** word, so a target expanding
to `/etc/x` is refused before anything is opened.

On the operator side, a redirection is made of **unquoted** bytes: the
descriptor digits and every operator byte. A quote byte inside them ends the
operator, so the word is an ordinary argument: `echo '&>f'` and `echo 2'>'g`
print their literal and open nothing, and `cmd '&>&name'` is an argument, not
a background task. A quoted byte *after* the first operator byte ends the
operator there, so `echo a > "&"f` opens the file `&f`. Quoted digits are not
a descriptor number, so `'2'>f` sends **stdout** to `f` (`>`'s default fd) —
with the digits not an argument: fdshell never splits a word at an operator
byte ([POSIX_COMPATIBILITY.md](POSIX_COMPATIBILITY.md#operators-are-recognized-at-word-starts)).

`set -o noclobber` (`set -C`) refuses a `>` that would overwrite an existing
file; `>|` is the explicit override. Several redirections may be written on
one command; they are applied in order, so the last one on a given descriptor
wins.

A redirection belongs to the command it is written on and is undone when the
command finishes — the shell's own descriptors come back. `exec >file` has no
command to run, so its redirection stays for the rest of the script.

A statement of **redirections only** — `>f`, `>>f`, `2>f`, `<f`, `>&2`, `>&-`,
`&>f`, `<<<x`, `>f 2>&1`, `>$(echo f)` — **is** a command (POSIX #2.9): the
redirections are applied, the shell's descriptors come back, and the status is
**0**. It runs in the current shell, so no process is launched and a
`NAME=value` prefix **persists** (`VAR=x >f` leaves `VAR=x` set, as bash does) —
there is no command for the prefix to scope to. The persistence is the shell's
own store only: with the command name omitted, POSIX #2.9.1 adjusts "the shell
variables but not the execution environment", so the assignment never *adds* the
export attribute — `VAR=x >f; printenv VAR` prints nothing — while a name
**already** visible to a child keeps its new value, whether the shell exported it
(`export VAR=y; VAR=x >f; printenv VAR` prints `x`) or the inherited environment
carries it (`HOME=y >f; printenv HOME` prints `y`). One side effect of that
in-process run stays an accepted divergence: a `set -x` trace prints nothing for
the statement (bash prints a line for the command it traces). A word behind the
redirections is the command word — `>f g` runs `g` with the redirection — and a
quoted operator word (`'>f'`, `"&>f"`) is a command name, never a redirection.
A **here-document operator** is a redirection too, so a statement that *begins*
with one is in this family: `<<EOF` + body, `3<<EOF` + body, `cat | <<EOF` + body
feed the body to the empty command's stdin, print nothing and exit **0** (see
*Here-documents*).

A word 0 that **expands to no word at all** is the same command (POSIX #2.9.1,
task #426): `$(true) >f`, `` `false` >f ``, `$unset >f` and `"$@" >f` with no
positionals leave the shell with an omitted command name, so the redirections
apply, nothing launches, and the status is **the last command substitution's** —
`$(false) >f` exits **1**, `` `false` `` exits 1, and the status a substitution
recorded for an *earlier* command never answers for this one (`$(false) >f;
$(true) >g` exits **0**). Nothing runs, so the status is **0** when no
substitution ran (`$unset >f`, `set --; "$@" >f`), and an async spelling keeps its
fork — a `&` job is a fork in bash too — with the arm running inside the job.
`$_` becomes empty, and everything else the paragraph
above says — the persisted `NAME=value` prefix and its shell-only store, the
silent `set -x` line, the pipeline stage that keeps its fork — is that same rule.
Only a *quoted* empty expansion stays a word, hence a lookup: `"" >f` and `"$(true)" >f`
are bash's `: command not found` at **127** against fdshell's shell-wide **1**
(an accepted divergence; the unquoted spellings are not words at all).

A **keyword block** takes a redirection written after its closing keyword and
applies it to the whole block: `while :; do echo a; break; done > f` files the
loop's output in `f`, and so does `until … done > f`, `if … fi > f`,
`for … done > f`, `for ((…)); do …; done > f` and `case … esac > f` — the glued
spelling `done>f`, `fi>f`, `esac>f` included, and a glued closer takes a **quoted**
operand too (`done>"f"`, task #413) — as does a closer glued behind a `case`
pattern word's `)` (`case x in a) echo hi;;b)esac>f` files nothing, task #421). The
scope opens **before** the block's own word list or condition runs, so a target
that fails to open stops
the block before its first command expands anything (`for x in $(touch m; echo
a); do :; done > /nope/x` leaves no `m`; the rc-1 abort is task #179's), and it
is undone at the block's end, so an `exec > z` inside the body dies with the
block. The tail follows the simple-command rules: several redirections apply in
order and the last one on a descriptor wins (`done > f > g` leaves `f` empty and
`g` the output), a word glued to an operator is one word — `done>f>g` opens the
file `f>g`, fdshell never splitting a word at an operator byte, as above. A plain
**word** behind the closer is a parse error, the way `group: unexpected word after
'}'` is one for `{ …; }`: bash demands a separator after a closing keyword, so
`done x`, `fi x#y` and `esac "c"` report it at fdshell's rc 1 (bash: rc 2) and
nothing of the block runs. The tail reads its operators **in source order** — path,
here-doc, here-string and the `&>` family — so `done <<T` feeds the body stream to the whole block
and `done <<<hi` feeds it its word, exactly as bash does (the `read`/`cat` commands
in the body are the ones that read it), and the io-number forms `done 3<<T` /
`done 3<<<hi` land on that descriptor. The `&>` family is read there too (task
#409): `done &> f`, `done &>> f` and their glued spellings land on the whole block
as one combined open shared by fd 1 and fd 2, so a body that writes to both streams
files them in one target, in bash's order. Two of the family's arms keep their own
answers at this position: the `%var` target stays the fd-variable reading documented
in POSIX_COMPATIBILITY (">%var and `&>%var` redirect to the descriptor"), where bash
reads the file name `%v`; and the pidvar spelling — `done &>&job`, `done &>|&job` —
is a **simple-command** position, so at a closer it is a parse error (bash: rc 2,
fdshell: rc 1). A `<<` **glued inside** the
closer's word (`done<<T`) stays word data, as above: the byte scan counts no
operator there, so nothing is claimed and the body lines run as commands — bash
feeds them to the block, and that row keeps its measured value.

### Here-documents

A here-document feeds a command's stdin from the lines that follow it, up to
a delimiter line:

```shell
cat <<EOF
line one
line two
EOF
```

A here-document **needs no command word**: when the operator is the statement's
first word, the statement is the redirections-only command above — the body feeds
the *empty* command's stdin, nothing prints, and the status is **0**, as bash:

```shell
<<EOF
the body is this empty command's stdin
EOF

cat | 3<<EOF
the body is fd 3's input in this stage
EOF
```

Every spelling joins it — `<<'EOF'`, `<<-EOF`, the io-number `3<<EOF` / `0<<EOF`
(its descriptor is the one opened), a statement that starts on a later line
(`true &&` + newline + `<<EOF`), a block arm (`if`, `for`, `while`, `case`,
`{ … }`), a pipeline stage (`cat | <<EOF`, `<<EOF | cat`), a background job
(`<<EOF >f &` creates the empty `f`), and a redirection written behind the
operator (`<<EOF >f` files nothing). A word behind the operator is the command
word — `<<EOF g` runs `g` with the body as its stdin — and a `NAME=value` prefix
**persists** through the statement (`VAR=x <<EOF` leaves `VAR` set, as bash).
Two shapes keep their own owners: `<<EOF` with **no body line at all** reports
the missing delimiter (rc 1, where bash reads to end-of-file with a warning), and
a `<<` **glued inside a word** (`x<<EOF`) stays word data
([POSIX_COMPATIBILITY.md](POSIX_COMPATIBILITY.md#operators-are-recognized-at-word-starts)).

An unquoted delimiter expands `$`/`$(…)`/backticks in the body. A **quoted**
delimiter makes the body literal, and any quote operator in the delimiter word
quotes it — `<<'EOF'`, `<<"EOF"`, `<<E\OF`, `<<EO\F`, `<<\E\O\F`, `<<\$HOME`
(POSIX XCU 2.3 counts `\` a quote operator, so the escape-only word `<<E\OF`
quotes the body exactly like `<<"EOF"`). The word is folded for matching, so
`<<E\OF` delimits `EOF`, and a quoted body is verbatim: nothing expands in it
and no `\<newline>` continuation is stripped from it. Every escape pair is a
byte **of the word**, so the whitespace or separator behind a `\` is delimiter
content: `<<a\ b` delimits `a b`, `<<a\<TAB>b` delimits `a<TAB>b`, `<<\;`
delimits `;` and `<<a\)` delimits `a)` (rc 0, as bash), and the pair quotes the
body — `X=v; cat <<a\ b` + `$X` + `a b` prints `$X`. A `\` at the **end** of the
word is the open member of that family: bash joins the word with the next line
(`<<E\` + `zzz` delimits `Ezzz`), which fdshell does not do yet. `<<-EOF` strips
leading tabs from the body and the delimiter. A here-document is usually
attached to `cat`, `read` or another command that reads stdin. A bare delimiter
word


that begins with `<`, `>`, `&` or `%` is a delimiter when that byte is
quoted (`cat << "<x"` + body + `<x` prints the body, as bash); unquoted it
is a syntax error. An **empty quoted** delimiter is a delimiter too — the
quote-removed word is the **empty** one, so the body ends at the **empty
line**: attached `<<""` / `<<''` and the bare `<< ""`, `<< ''`, `<<- ""`,
`3<< ""` all take the following lines as a literal body, and `cat << ""` +
`body` + a blank line prints `body` (rc 0, as bash). In the `<<-` form a
tabs-only line is such an empty line, so it terminates the body.

A body line is **data**, at every here-document site: quoting, word
splitting, globbing, brace expansion, block keywords and every other pre-parse
rewrite pass over it untouched, and only `$`/`$(…)`/backquotes expand in it (an
unquoted delimiter). So `cat <<T` with the body line `{a,b}` prints `{a,b}`, and
so does the same here-document inside a single-line keyword block's arm —
`if true; then cat <<T; fi` + `{a,b}` + `T` prints `{a,b}`.

The **delimiter line** is heredoc structure, not a command line: its
metacharacters, quotes and expansion bytes are inert, so a delimiter word may
contain them and its own terminator line is matched literally —
`cat <<'a <<b'` + `body` + `a <<b`, `cat <<'a|b'` + `body` + `a|b` and
`cat <<a\"b` + `body` + `a"b` all print `body` (rc 0, as bash). Its words stay,
so a block keeps the delimiter line inside its own text and the statement
written behind a terminator (`cat <<'a <<b' && echo after` + body + terminator)
is a separate statement. The word's own `$( … )` region is part of the word — the
byte walk consumes it whole, exactly as it consumes a quoted region — so
`cat <<$(x)` + `body` + `$(x)` + `echo after` prints `body` and `after` (rc 0, as
bash), and a block arm whose terminator line is such a word keeps the whole
terminator line inside its own text.

A delimiter word that is **brace-shaped** is structure too, so it is not brace
syntax: `f() { cat <<}` + `x` + `}` + `}` + `f` prints `x` (rc 0, as bash), and
so does the bare spelling `f() { cat << }` — a `}` operand does not close the
definition and a `{` operand does not open it, in the `{ …; }` group alike; the
def matrix `a_brace_shaped_delimiter_word_of_a_function_body_is_structure` and
the group matrix `a_brace_shaped_delimiter_word_of_a_group_body_is_structure`
pin it. Such a word stays a token, and it scores as nothing at the brace rules —
the byte scan takes an operator word and its operand as one unit, the shared
closer walk never scores it, and a brace-shaped word in an ordinary **argument**
position still counts as brace syntax.

A physical line owns **one** body stream, and its regions are handed out in
operator order to the operators written on that line: the pre-block statements
first, then the block's own operators, then the trailing statements. A
multi-line `name() { … }` definition claims none — its body's here-docs feed the
statements inside it: `cat <<X; f() { echo hi; }; f; cat <<B` + `bodyX`/`X`/
`bodyB`/`B` prints `bodyX`, `hi`, `bodyB`. A `;`-terminated operator written in
a single-line **block**, definer or `{ …; }` group body claims its own region of
that stream and the body text carries it: `f() { cat <<T; }; f` + `body` + `T`
and `{ cat <<T; }` + `body` + `T` both print `body`, and the region lines are
data, never commands. A **block opened mid-run**
(`true && while :; do cat <<T; … ; done` + body + delimiter) travels with the
run, so the part that owns the `<<` gets its body and prints it, as bash; what
stays a measured divergence is the multi-line construct whose region swallows
its own closer (`true && while :; do` + newline + `cat <<T; break` + `done` +
body + delimiter): rc 1 with the construct's unterminated-shape message, where
bash is rc 2.

The cond-list operator written in a here-doc body region is a split point only
where the `<<` sits in a **block's clause** (`if cat <<X; then` + `body && x` +
`X` + `fi`: rc 1 `missing 'fi'`, where bash reports a syntax error at rc 2 — the
accepted rc-1-vs-2 shape); a region claimed by a word of the block's
**body** is the body's own data — `if :; then cat <<Q` + `z && y` + `Q` + `fi`
prints `z && y` at rc 0, as bash, in every block shape: `then`/`else` arms,
`for`/`while`/`until`, `case` arms, nested blocks, a `{ …; }` group
(`tests/heredoc.rs::and_in_a_body_position_heredoc_region_is_body_data`,
`a_body_position_heredoc_region_in_every_block_shape_prints_its_body`, and
`a_clause_position_region_still_splits_the_block_text` for the clause rows).

The `)` row of that rule is **gated on the kind of the open block** (task #430):
a `)`-ending word ends the clause only in a block whose clause *ends* with an arm
`)` — a `case` arm and the event-`wait` block — so in an `if` clause such a word
moves nothing and the region behind it stays scanned: `if (cat) && cat <<X; then`
+ `z && y` + `X` + `fi` is rc 1 `missing 'fi'`, bash rc 2
(`tests/heredoc.rs::a_word_ending_in_close_paren_in_an_if_clause_splits_the_region_again`,
unit pins in `cond/boundary/block/clause/tests.rs` and
`keywords/position/tests.rs::the_clause_keyword_set_is_one_set_with_after`). Two
rows of that family stay divergent **by rule**, both because their body answer
arrives one word later, through `keywords/position`'s `)` row — which stays
ungated, since it answers command position: `if (echo a); then cat <<X; then` +
`z && y` + `X` + `fi` runs at rc 0 and `if :; then echo a); cat <<X` + `z && y` +
`X` + `fi` prints both lines at rc 0, where bash is rc 2 at both
(same pin).

The stream is read **greedily, one body per operator**, as bash reads it: each
operator's body starts where the previous operator's delimiter line ended, and
an operator whose delimiter line never arrives — **the first one included** —
takes what is left of the stream as its body (empty when nothing is left) and
runs at rc 0, printing the end-of-file warning
`warning: here-doc delimited by end-of-file (wanted 'E')` (fdshell's wording,
no line numbers; bash also warns for operators in branches that never run,
where fdshell stays silent — pinned in `tests/heredoc.rs`). The operators
behind it get an empty body, and the body lines never run as commands. So
`cat <<T; cat <<E` + `bodyT`/`T`/`extra` prints `bodyT` and `extra` (rc 0), and
`if true; then cat <<T; elif cat <<E; then echo b; fi` + `bodyT`/`T` prints
`bodyT` (rc 0); `cat <<E` + `body` alone prints `body` with the warning.

A number in front of the operator puts the body on that descriptor instead of
stdin: `cat 3<<EOF` makes the body fd 3's input and leaves stdin alone, and
`exec 3<<EOF` keeps it there for the rest of the script. The descriptor it opens
is **read-only**, as bash's here-document source is: `exec 3<<EOF` + body +
`echo hi >&3` reports `echo: write error: Bad file descriptor` and leaves the
body intact (`cat <&3` still prints it). The number must be its own unquoted
digit word, as in `3>file` — `cat "3"<<EOF` and `cat \3<<EOF` are
the single argument `3<<EOF`, its quotes taken as ordinary word quoting and
its `<<` as plain word text, not a here-document
([POSIX_COMPATIBILITY.md](POSIX_COMPATIBILITY.md#here-document-and-here-string-descriptor-numbers)).
`N` is a raw descriptor number, not an fd variable: it has no `%fd`
counterpart — nothing is bound under a name, `unset %name` closes nothing —
and the descriptor is reachable only as a number in the same command's
redirections (`3<<EOF <&3`, `exec 4>&3`).

A here-string feeds one expanded word plus a newline:

```shell
read -u 0 LINE <<<"hello $name"
```

A quoted word that begins with `<`, `>`, `&` or `%` is that word:
`cat <<< "<x"` prints `<x`, as bash; an unquoted one is a syntax error.

A number in front of the operator puts the word on that descriptor instead of
stdin, under the here-doc's raw-byte number rule: `cat 3<<<text` makes `text\n`
fd 3's input and leaves stdin alone, and `exec 3<<<text` keeps it there for the
rest of the script (`exec 3<<<text; cat <&3` prints `text`). The descriptor it
opens is **read-only**, as the here-document's is: `echo hi >&3` after it
reports `echo: write error: Bad file descriptor` and the body survives. The
number must be its own unquoted digit word, as in `3>file` — `cat "3"<<<x`,
`cat 3"<<<"x` and `cat \3<<<x` are the single argument `3<<<x`, its quotes
taken as ordinary word
quoting and its `<<<` as plain word text, not a here-string
([POSIX_COMPATIBILITY.md](POSIX_COMPATIBILITY.md#here-document-and-here-string-descriptor-numbers)).
`N` is a raw descriptor number, not an fd variable: it has no `%fd` counterpart
and is reachable only as a number in the same command's redirections, or from
the shell's own table after an `exec`.

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
echo ~            # $HOME        (a quoted `~` is a literal byte: `echo "~"`)
echo ~/x          # $HOME/x
echo ~"/*"        # ~/*          (a quoted character in the tilde-prefix —
                                 # the bytes up to the first `/`, the `/`
                                 # included — suppresses tilde expansion)
echo ~""          # ~            (a quote pair counts as a quoted character
                                 # even when it contributes no bytes: `~""`,
                                 # `~''`, `~""/x` and `""~/x` all stay literal,
                                 # while `~/""` expands — the pair sits after
                                 # the prefix's end)
echo ~user/x      # ~user/x      (no password database: `~user` stays literal)
```

`~` and `~/x` read `HOME` through the **shell variable** — the same lookup
`$HOME` uses: the shell's own variables first, then the environment fdshell
was started with. A `HOME=/tmp/H` assignment therefore moves `~` exactly as it
moves `$HOME` (`echo ~/x` → `/tmp/H/x`), and an empty `HOME` leaves the rest of
the word (`HOME=; echo ~/x` → `/x`). When `HOME` is set in neither table `~`
contributes nothing, where bash falls back to a login directory from the
password database — an accepted divergence, like `~user` above.

A **quoted word is never a keyword** (POSIX §2.9.1): the block keywords are
recognized only on a word whose own span carries no quote byte, so every
spelling of a keyword with quotes is a command name — `'if'`, `"if"`, `i"f"`,
`""if`, `''if` and `"i"f` all look up the command `if`, and the same holds for
`while`, `until`, `for`, `case`, `{`, `function`, `then`/`else`/`elif`/`do`/`in`
and the `name(` definer of a function definition:

```shell
'if' x        # command `if`: not found (fdshell exits 1; bash 127)
f'oo'() { :; } # not a definition: the word `foo(` is not a valid name
```

A `\`-escaped word (`\if`) was never a keyword here: its text is `\if`, so it
is looked up in `PATH` in both shells. The command-not-found exit code is
fdshell's shell-wide **1**, not bash's 127. The same word-unit rule covers a
`$( … )` region: the region is one word, so its `(` is never the definer
opener's `(`. A region word the definer production does **not** match
(`$( echo true ) { :; }`, `x=$( echo a ) { echo b; }`) is a simple command and
runs its words; bash's rc 2 there is its stray-`}`-at-command-position rule
(task #499), not the substitution. A region word the production **does** match —
the word ends with its own `(` and the `)` word follows, or `function` is
followed by the name word — is a name slot, and fdshell reports it:
`'$( echo f )(' is not a valid function name (a command substitution is not part
of a definer word)`, rc 1 with nothing run, where bash reports its own
`` `$(echo f)': not a valid identifier `` and keeps going. An unclosed block
names the keyword it wants —
`'if' is missing its closing 'fi'` — and fails with that same parse-error rc 1.

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

`unset name…` removes each named shell variable — its value, its export
attribute and the inherited environment row it came from, all three, so the
name leaves `$name`, `${name:-word}` and the child environment together (after
`unset HOME` the bare `cd` reports `HOME not set`). The `%` spelling stays
fdshell's fd form: `unset %name` closes the fd variable (descriptor, fd array,
named task) and `unset %arr[%src]` removes one array entry. An unbound name is
a silent no-op with exit status 0, a bare `unset` does nothing, and
`unset IFS` restores the default split set (space, tab, newline) while the name
itself becomes unbound — it reads empty, and `set -u` reports it (bash). No
operand is validated: `unset 1abc` names a variable no expansion can reach and
exits 0, which is what bash does too.

The name namespaces follow bash's no-option rule: **variable first, function
second**. A name that is also a function keeps its function — the variable is
what this reaches first — and a name that is not a variable removes a function of
that name, so `f(){ :; }; unset f; f` runs nothing. An unbound name stays the same
silent no-op. The operand is never an option here: `-f` and `-v` are read as names
(bash's `unset -f`, `-v` and `-x` rows are documented in
[POSIX_COMPATIBILITY.md](POSIX_COMPATIBILITY.md) by task #387). The rule is pinned
in `tests/unset.rs::a_bare_unset_names_a_variable_first_and_a_function_second`.

`name=(words)` is a **string array** assignment (task #422): each word inside the
parentheses is one element, on its own through the ordinary word pipeline
(substitution, `IFS` splitting, glob), so `a=($(echo 1 2))` has two elements and
`a=("*")` has one. `name+=(words)` appends, `name[idx]=value` writes one element,
and `unset name` takes the whole store entry. One name is one namespace: a plain
write to a name the store holds replaces element 0, and a scalar answers index 0
(`a=hello; echo "${a[0]}"` is `hello`), so the two writers cannot drift.
`local name` and `declare` do not scope the store yet, and the `%` namespace
stays fdshell's fd form (`echo "[%a]"` prints `%a`, never the array). Pinned in
`tests/arrays.rs`.

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
| `${name[idx]}` | the element of a string array (a mere scalar answers index 0) |
| `${name[@]}` / `${name[*]}` | the elements as separate words / joined with the first `IFS` byte |
| `${#name[@]}` / `${#name[idx]}` | the element count / the length of one element |

The array reads obey the same two join rules as the positional ones (LESSONS:
"$@ has two join rules, selected by the trigger byte's quote bit") — a fully
quoted `[@]` keeps every element a field of its own, a fully quoted `[*]` joins
them into one, and an embedded arm agrees with the whole-word one; the empty
array keeps the positional contract of task #219 (`"${a[@]}"` yields no field at
all, `"${a[*]}"` one empty field). `[@]`/`[*]` never bail under `set -u` (an
unset name gives nothing), an unresolved `[idx]` does. Pinned in
`tests/arrays.rs`.

The positional and special parameters are also available: `$1`, `$2`, … are
the arguments, `$#` the argument count, `$0` the program name, `$?` the last
status, `$$` the shell pid, `$!` the last background pid. `"$@"` expands to
the arguments as separate words; `"$*"` joins them with the first `IFS` byte.

A **bare** name — `$v`, `$1`, `%fd` — ends at the closing quote of the quote
region that holds the trigger byte, as it does in bash: `"$v"bc` is `v` plus the
literal `bc`, `"$v""b"` is `v` plus `b` (the second `"` opens a new region),
`"$1""2"` is `$1` plus `2`, and `"$v"'x'y` is the value of `v` plus `x` plus `y`.
Inside one region the name stays greedy (`"x$vbc"` is the value of `vbc`), and a
braced reference written **inside** one region is unaffected — `${name}` ends at
its `}`; a region that ends at the `$` leaves no braced reference either, so
`"$"{v}` prints `${v}` (bash parity).

The cap covers **every** `$` arm, special parameters included: a `$` whose own
region ends at the `$` starts no reference, so the `$` is a literal byte —
`"$"?` prints `$?`, `"$"#` prints `$#`, `"$"@` prints `$@`, `"$"!` prints `$!`,
`"$"$` prints `$$`, `"$"-$` prints `$-$`, and `x"$""$"` prints `x$$`; bash
prints each of them the same way
(`tests/quotes.rs::special_parameter_arms_obey_the_region_cap`).

A digit run obeys the **same** cap, so a region that ends right after the `$`
leaves no name and no digit: the arm consumes nothing, the `$` is a literal
byte, and the text after it is literal text. That is the one reading of the
`$'…'` and `$"…"` spellings, an **accepted divergence** from bash, which treats
them as an ANSI-C / locale translation and expands inside the span:

```shell
printf "%s" $'v'     # $v     (bash v)
printf "%s" $"v"     # $v     (bash v)
printf "%s" $"1"     # $1     (bash 1, positional 1)
printf "%s" $"12"    # $12    (bash 12)
printf "%s" "$"12    # $12    (bash prints $12 too: parity)
```

Because nothing there is a parameter reference, `set -u` has nothing to report
and the status stays 0.

### Command substitution

`$(cmd)` and `` `cmd` `` run `cmd` and substitute its output:

```shell
files=$(ls)
echo "there are $(echo "$files" | wc -l) entries"
```

A `$( … )` region is **one word**, at every level of the scanner, so whitespace
inside it is body text and never breaks the word: a body that starts with a
space is an ordinary command word and an ordinary assignment value.

```shell
$( echo true )       # runs `true` (word 0), not a parse error
$( echo echo ) hi    # hi
x=$( echo a )$( echo b )   # x is `ab`
```

Word 0 is where an expansion can **vanish**: `$(true)`, `` `false` ``, `$unset` and
`"$@"` with no positionals produce no word at all, so bash's command name is
*omitted* — nothing runs, a redirection written on the same command still applies,
and the status is the last substitution's (`$(false) >f` exits **1**, `$(true) >f`
exits 0). A *quoted* expansion that comes out empty (`"$(true)"`) is a word, so it
is the empty name and a lookup. The whole rule is under
[Redirection](#redirection) (POSIX #2.9.1, task #426).

A word that carries a substitution region is never the `name(` definer of a
function definition — the shapes bash writes as definitions carry no region. An
unterminated region (`$( echo true`) is reported as `unexpected end of input`
with the caret, rc 1.

The region expands in every word position the shell expands: a command name
(word 0), an argument, an assignment value, a `for` list, a here-string word,
a **redirection target** (`echo hi > $(echo f)` writes the file `f`, see
[Redirection](#redirection)), and `cd`'s operand (`d=sub; cd $d` moves the
base, see [Word splitting and pathname
expansion](#word-splitting-and-pathname-expansion)). The here-document
**delimiter** word is the exception — it is quote-removal only, so
`cat <<$(echo EOF)` delimits on the literal `$(echo EOF)`.

### Word splitting and pathname expansion

The result of an expansion is split into words on the `IFS` bytes (space, tab
and newline by default), and then each word containing `*`, `?` or `[...]` is
expanded against the filesystem:

```shell
IFS=:; parts=$PATH        # split PATH into a : separated list
for f in *.txt; do …; done
```

The shell **sets `IFS` to that default at startup**, even when the parent
exported it (POSIX XCU 2.5.1, bash): an inherited `IFS` contributes only its
**export attribute**, so `IFS=, fdshell -c 'builtin echo "[$IFS]"'` prints the
default split set, and a child's `env` shows `IFS=` with the default value as
well. Only an assignment inside the shell changes the value; with no inherited
row, `env` shows no `IFS=` line at all.

A quoted byte never splits and never globs. `set -f` disables pathname
expansion; `shopt -s dotglob` lets patterns match a leading dot;
`shopt -s nullglob` deletes a pattern with no match; `shopt -s failglob`
makes it an error. `[!…]` and `[^…]` negate a bracket expression (bash accepts
both markers; POSIX defines only `!`).

A `case` **pattern** gets neither treatment: it is one word, never split on the
`IFS` and never globbed, and only its **first expanded word** is the pattern —
so a quoted `"$@"` in a pattern is the **first positional** (bash keeps
`es->word->word`), an unquoted `$@` space-joins, and `"$*"` joins with the
first `IFS` byte (concatenating when the `IFS` is empty). The braced spellings
`${@}` / `${*}` obey that same matrix — on this surface bash's braced spelling
equals the unbraced one. One measured divergence is pinned: with an empty `IFS`
and an empty positional (`IFS=; set -- a ""`) bash's pattern text carries its
quoted-null byte (0x7f) and matches nothing, where fdshell space-joins.

`cd`'s operand is an ordinary word, so every rule above applies to it: the
target is the **expansion**, not the text the user typed.

```shell
V=sub; cd $V             # sub
cd "$(echo sub)"         # sub      (`"` never suppresses a substitution)
cd '$(echo sub)'         # the literal name, which fails
cd ~                     # $HOME    (a quoted `~` is a literal byte)
cd $((1+1))              # the directory named `2`
set -- sub; cd "$@"      # sub
cd a b; cd sub extra     # cd: too many arguments
cd "a b"                 # one path with a space in it
cd s*                    # the one match of `s*`
cd *                     # cd: too many arguments (several matches)
```

Two fdshell rules sit on top of the expansion. The `%name` form is read on the
**raw** word, before any expansion, and quoting does not exempt it: `cd %dir`
is the stored fd ([POSIX_COMPATIBILITY.md](POSIX_COMPATIBILITY.md#is-a-file-descriptor-namespace)),
while the pipeline's `%` arm yields the fd *number* — expanding the word would
turn `cd %dir` into `cd 7` and lose the fd's origin. `cd \%d` folds the escape
pair, so it is the literal name `%d`, as in bash. And an operand whose
expansion produces **no field at all** — `cd "$@"` with no positionals,
`cd $unset`, `cd $(true)`, `shopt -s nullglob; cd zzz*` — is the bare `cd`:
`$HOME`. The `-` form is read on the expanded field, so `D=-; cd "$D"` returns
to the previous directory even where a directory named `-` exists; that
directory is reached by any other path, `cd ./-`.

A `-` operand also prints: after a **successful** move, `cd -` writes the new
directory to standard output, as POSIX XCU 4.57 requires (bash does the same).
fdshell writes the value its own `pwd` writes — `getcwd` is the shell's only
source for the cwd's path — where bash writes its new `PWD` value; the two agree
byte for byte on every path reached without a symlink hop, and `cd`'s
`O_NOFOLLOW` open means fdshell never takes one. A failed move prints nothing,
and its report aborts the script (the task #179 family, where bash continues).

`$_` reports the last **expanded** field the command consumed: `V=sub; cd $V`
leaves `$_` at `sub`, and the quoted twin `cd '$V'` leaves it at the literal
`$V`, because a quoted word is not expanded. An option word is a field too, so
`cd --` leaves `$_` at `--` and `cd -- sub` at `sub`. A fully quoted word that expands
to nothing counts as an (empty) field, and the command word is the `$_` only when
there were no operand words at all. Bash sets `$_` the same way for every command
it runs, and every intercept that expands its words (`hash`, `cd`, `eval`,
`source`, `local`, `let`, `wait`) hands its field list to that one frame;
`set --` keeps its own store of the expanded list.

The bare `cd` reads `HOME` through the **same lookup `$HOME` and `~` use**: the
shell's own variables first, then the environment fdshell was started with. A
`HOME=/tmp/H` assignment therefore moves the bare `cd` exactly as it moves
`$HOME` and `~`, and a `local HOME=/tmp/H` shadow moves it inside the function;
the scoped `HOME=/tmp/H cd` prefix moves it too and is undone after the
command, as in bash. Set in neither table the report is `$HOME not set` at
rc 1 — bash's `cd` answers `cd: HOME not set` at the same rc, so here the two
agree: bash's password-database login directory is `~`'s fallback, not `cd`'s.

The accepted divergences of the same shape: two or more fields is
`cd: too many arguments` at fdshell's rc 1 and **aborts the script** (bash
returns 2 and the script continues, the task #179 family); `cd ""` is the open
failure on the empty path, where bash says `cd: null directory`, and the
empty-`HOME` form `HOME=; cd` is that same open failure at rc 1, where bash
leaves the directory alone at rc 0; a scoped `IFS=` prefix splits the
expansion on the *scoped* `IFS` (`v=a:b; IFS=: cd $v`
is two fields here, one name in bash — the
`tests/cmd_subst.rs::word0_scoped_ifs_divergence` family).

`cd`'s option words are parsed on the **expanded fields**, from the head of the
list, exactly as `hash` parses its own: `--`, `-L`, `-P` and `-e` are accepted —
quoting is no gate — and the last three are no-ops, because the move is a pinned
`O_DIRECTORY + O_NOFOLLOW` fd and the logical/physical distinction bash measures
on a symlinked directory has nothing to decide here (the `-L -P` conflict bash
accepts is accepted too). `--` is consumed and **ends** the parsing, so every
word after it is an operand, and a lone `-` is not an option: it is the `OLDPWD`
form. The first other word stops the parsing, so the words after it are counted
by the rule above.

```shell
cd --                     # $HOME    (`--` ends the options)
cd -- sub                 # sub      (the operand after the terminator)
V=-L; W=sub; cd "$V" "$W" # sub      (options are parsed after expansion)
cd -- -L                  # the open failure: `-L` is a name after `--`
cd sub -L                 # cd: too many arguments
```

Pinned divergences of this surface: bash accepts a clustered short (`-LP sub`
cds) where fdshell matches the four words whole, and bash's message names the
first bad character (`---` is reported as `--`) where fdshell names the whole
word; bash's invalid option is a usage line at rc 2 and the script continues,
where fdshell prints no usage line and reports at rc 1 — the family above. bash's
help line `[-@]` is the getopt magic-`@` marker, not an option, so `cd -@` is the
invalid option in both shells. A failed `cd` still sets bash's `$_` to its last
word (`cd -x`), which fdshell cannot show end to end because its report aborts
the run. The `%name` gate stays a **word 0** rule, so in `cd -L %dir` the `%`
word goes through the pipeline like any other argument.

### Command lookup table (`hash`)

`hash` is the table of resolved program paths: a successful `PATH` search
records its result there, and a later run of the same name skips the scan.

```shell
hash                         # list the table, one `name<TAB>path` line per entry
hash ls                      # look `ls` up, print the path, cache it
hash mytool /opt/bin/mytool  # pin the name to a path (fdshell extension)
hash -r                      # clear the table
hash -r mytool               # drop one entry (fdshell extension)
hash -d ls                   # drop one entry (bash's `-d`)
```

Every word `hash` consumes is an ordinary word: `$name`, `$( … )`, backticks,
`$(( … ))`, `~`, `%fd`, `IFS` splitting and pathname expansion all apply to
the name words and to the pin path, with the usual quoting rule (`"` never
suppresses an expansion, `'` does) and the usual glob policy (`set -f`,
`shopt -s nullglob`/`failglob`/`dotglob`).

```shell
hash $(echo ls)              # the name `ls`
hash '$(echo ls)'            # the literal name `$(echo ls)`
hash *                       # one name per match in the current directory
set -f; hash *               # with `noglob`, the pattern itself
```

The option word is read **after** the expansion, so `hash $(echo -r)` clears
the table. An expansion-free name folds its escape pairs as every word does,
so `hash a\*` looks up the name `a*` (and its `*` is protected, so it never
globs). The table is keyed by the **expanded** name — the shape the shell
stores when it runs a command, so the two keys meet. 0 fields is the bare
listing: `hash $(true)` and `hash $@` with no positionals print the table
instead of looking up an empty name.

fdshell's accepted divergences from bash, all owned by task #375 (the
`-l`/`-t`/`-p` option and listing surface): `hash NAME PATH` pins the entry
(bash hashes each name), `hash -r NAME…` removes those entries (bash ignores
the names), `hash NAME` prints the resolved path (bash prints nothing), a
bare `hash` of an empty table prints nothing (bash says
`hash: hash table empty`), a name with a `/` reports `hash: NAME: not found`
(bash accepts it silently), a third field is `hash: bad usage …` (bash
hashes each name), and a bare `hash -d` is the same usage error (bash's
getopt diagnostic). `hash -d NAME` on a name with no entry reports
`hash: NAME: not found` and exits 1, as bash does.

### Brace expansion

`{a,b,c}` produces one word per alternative, and `{1..5}` a range:

```shell
echo {foo,bar}.txt        # foo.txt bar.txt
echo img{01..03}.png      # img01.png img02.png img03.png
```

Quoted braces are literal.

A redirection **operand** is the one word bash counts (POSIX XCU 2.7 `worddef`):
a brace group there that expands to more than one word — or to none — is
`ambiguous redirect` (rc 1); the command runs nothing and opens nothing, as
bash. The count is of **non-empty** words and the operand's own quotes are
kept, so `> {a,b}`, `>>{a,b}`, `<{a,b}`, `> x{a,b}`, `>{1..3}`, `&>{a,b}`,
`2> {a,b}`, `<&{3,4}`, `>%{a,b}` and the zero-word `>{,}` all bail, while
`>{a}`, `>{a..a}`, `>{ 1 , 2 }` and `>'{a,b}'` open their one word.

The residuals of the pre-parse rewrite stay (see "Operators are recognized at
word starts" in [POSIX_COMPATIBILITY.md](POSIX_COMPATIBILITY.md)): braces in an
io-number **prefix** still produce several redirect words, so `{3,4}>f` runs one
`cat` with fds 3 and 4 on `f` where bash runs `cat 3` and `cat 4`; a redirect
that **precedes** the ambiguous one is left unopened, where bash opens it
(`echo x >out >{a,b}` creates no `out`); the attached one-non-empty-word form
`>{,zz}` is a parse error where bash opens `zz`; and a failed redirection aborts
the **script**, where bash skips only the command (the async form
`echo x >{a,b} &` is rc 1 in fdshell, where bash's own status is 0).

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

`$name`, `$$`, `$!`, `$(…)`, nested `$((…))` and `${…}` regions expand inside
an expression before it is parsed (bash's model: expand first, then evaluate
the resulting string) — on every surface at once: `$((…))`, the `((expr))`
command, `let` and the arithmetic `for`'s clauses. A braced region goes through
the word expansion path, so its param-ops (`${n:-2}`, `${n#a}`, `${#n}`) and
braced positionals (`${1}` is `3` with `set -- 3 4`, as in bash) work as bash
does. v1 decisions (task #397, implementation task #517): a bare `$1` or `$*`
inside an expression stays a syntax error (bash expands them, `3` measured),
and an unset variable inside a region splices to empty — fdshell reports its
own syntax error at rc 1 where bash's empty-expression rule prints `0` at rc 0.

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
for name; do echo "$name"; done
for ((i = 0; i < 3; i++)); do :; done
while read line; do echo "$line"; done
until [ -e ready ]; do sleep 1; done

case "$1" in
    start) echo starting ;;
    stop|halt) echo stopping ;;
    *) echo "unknown" ;;
esac
```

A redirection written after a block's closing keyword belongs to the **whole**
block: `for x in a b; do echo $x; done > f` files the loop's output in `f`, the
glued `done>f` included, and the block's own fd changes are undone when it ends
(see "Redirection" above). The `&>` family is part of that tail (task #409):
`done &> f` and `done &>> f` write the block's **two** streams through the one open
the operator stands for, and its pidvar spelling (`done &>&job`) is a parse error at
a closer — the pidvar is a simple-command position (POSIX_COMPATIBILITY "`&` names
background tasks").

A block's **condition** must be a non-empty list: `if ; then echo x; fi`,
`elif ; then`, `while ; do :; done`, `until ; do :; done` and their `&` and
newline spellings (`if` newline `then`, `if & then`, `while & do`) are parse
errors — bash stops with rc 2, fdshell with rc 1 and its own message
(`missing condition` for `if`/`elif`, `expected condition` for the loops). An
empty condition is *true* at run time, so `while ; do echo x; done` would never
end. The rule covers conditions only: a `for` **word list** may be empty
(`for i in; do echo x; done` iterates zero times, bash rc 0), and an empty
`for ((…))` arithmetic condition is true, as in bash. The `case` **word** is not
a condition but has its own rule: POSIX #2.9.1 requires a `word` between `case`
and `in`, so `case; in esac`, `case ; in esac`, `case in esac`, `case` + newline
`in esac` and a comment-only word are parse errors (bash rc 2, here rc 1 with
`case: empty word`) — while a quoted empty pair and a quoted `";"` **are** words,
and those spellings run at rc 0 as bash does.

A `for` **word list** is read as **words**: no keyword scores inside it, so
`for i in done`, `for i in esac`, `for i in { 1 2` and `for i in }` are ordinary
words and the block runs from its own `do` to its own `done` —
`for i in done; do echo "$i"; done` prints `done`, at rc 0 as in bash. A `case`
**pattern list** is the opposite place: `esac` **terminates** it, so
`case x in esac` is a complete `case`, while `case done in done) echo p;; esac`
is fdshell's parse error (task #310). A list word glued to an operator is word
data here, so `for x in done>f; do echo A; done>f2` files `A` in `f2` and
`for i in x& ; do echo "$i"; done` prints `x&` — both at rc 0 where bash
stops at rc 2 (accepted divergences). The **standalone** operator words are the
ones the list rejects, by one predicate shared with the `case` pattern list (task
#524): `&` (task #360), `&&`, `||`, the pipeline `|` and a `;` that is not the
list's own terminator before `do` — bash's list ends at an operator and its parser
fails the `do` expectation there, so `for i in 1 & ; do :; done`,
`for i in 1 && ; do :; done`, `for i in 1 | ; do :; done` and
`for i in ; 1; do :; done` are parse errors (rc 1 here, rc 2 in bash), while
`"&"`, `'&'`, `"&&"` and `\&` stay words; the glued family above is the surviving
divergence (owner #496), and the parens stay words for
task #470. A missing `in` stays fdshell's own rc 1:
`for i 1 2` is `expected 'in'`, bash rc 2.

The `in word-list` of a `for` clause is **optional** (POSIX #2.9.1), and a clause
without it — `for name; do …`, or the same clause with a newline before `do` —
iterates the **positional parameters**: one word per parameter, verbatim (no IFS
split, no pathname expansion), exactly the words `"$@"` expands to, and no word
at all when none are set. The empty list right beside it is a **different** row —
`for name in; do` iterates zero times — so the parse keeps the two apart as a
field of the block. A keyword written in the name slot is the name, and bash
iterates the parameters there too (`for in; do`, `for fi; do`, `for then; do`);
only the block's **own** closer stays fdshell's parse error, bash rc 0
(task #522).

A block's **closing keyword** needs a command-list terminator in front of it —
POSIX #2.9's `;` (a newline is one) or the plain `&` — with the one exception
bash makes: the terminator after a **compound command** is optional, so a body
whose last statement is an `if`, a loop, a `case`, a group, a subshell or a
function definition closes straight at its own keyword. `while false; do if :;
then :; fi done`, `for i in a; do if :; then echo f; fi done`,
`if true; then { echo a; } fi`, `if true; then f() { :; } fi` and
`while false; do ( : ) done` (a subshell's `)` is a closer too; a subshell that
**runs** is task #316's gap), mid-run (`true && if …; fi`) and as a pipeline
stage (`echo a | while …; done`) alike, all run at rc 0. The rule reaches a
function definition's **own** `}` too (task **#424**): `f() { echo a }` reports
`expected ';' before '}'` — the group's message, reused — at rc 1 where bash
stops at rc 2, and nothing of the statement runs (`f() { echo a } ; f` prints
nothing, `f() { echo a } >f` opens no file), while the compound-command exception
applies at the def site as everywhere else, so `f() { if :; then :; fi }` and
`f() { { :; } }` run at rc 0; the glued spellings the tokenizer never reads as a
def (`f() {}`, `f() {:;}`) and the comment-line predecessor — the group arm's half
of the set, so a def body ending on one reports where bash runs — are pinned as
measured in `safe/fdshell/tests/block_bodies.rs`. A block keyword written as an
**argument** is word data and never swallows the closer, so
`if true; then echo a else fi` and `while false; do echo a else done` are parse
errors (bash rc 2, fdshell rc 1 with its own
`'if' is missing its closing 'fi'`). A closer glued straight behind a clause
keyword reports the missing `;` first — `if true; then fi` is
`expected ';' before 'fi'`, `while true; do done` is
`expected ';' before 'done'` (the message `for` pinned first) — while a real
`;`/newline in front of the closer keeps the empty-body message
(`if true; then; fi`). The rest of the family stays fdshell's own rc-1 parse
error where bash stops at rc 2: `while :; do : done`, `if true; then b fi`,
`for i in a; do echo done`, `if true; then echo a >f fi`, and a closer glued to
a redirection (`while false; do { :; }>f done`). The closing keyword is the one
place the glue is read the way bash reads it: the operand behind it
(`done>f`, `fi>f`, `esac>f`, task #413) **and** the byte in front of it — a `case`
pattern word's `)` (`case x in a)esac`, task **#421**) — so those shapes run at
rc 0. A `;;` written in a `then` body is the stray-`;;` rule below, not this
one's: `if true; then echo a ;; echo b; fi` reports (rc 1, nothing prints, bash
rc 2). Its **one** exception is this rule's: a pair written immediately in front
of the closer, where the token in front of it is claimed as POSIX's `;` and the
body the guard then reads holds a single `;` and no pair —
`if true; then echo a ;; fi` prints `a` at rc 0, and so does its spaced twin
`if true; then echo a ; ; fi`; what the claim leaves behind still parses, so
`if true; then b ;; fi` ends at its runtime miss `"b" not found`. All three are
pinned as measured
(`safe/fdshell/tests/block_closers.rs::a_double_semi_body_is_the_separator_rule_not_this_one`,
the spaced twin in `safe/fdshell/tests/semi_semi.rs`), bash rejecting each at rc 2.

This rule answers only the closer's **predecessor** question, so a body that
ends in a **dangling `&&`/`||`** behind a legal terminator reaches its own
condition list — and that list reports (task #367, rc 1 where bash stops at rc
2): `if true; then echo a ||; fi`, `if true; then echo a ||` + newline + `fi`,
`for i in 1; do echo a ||` + newline + `done`, `{ echo a ||; }`,
`f() { echo a || }; f` — the function body's missing terminator before `}` of
#424, whose row reports through this rule now — and `case x in x) echo a || ;;
esac`, the same for the `;;` rule of #423. A body whose list *runs* stays
bash-parity: `while false; do echo a ||` + newline + `done` never reaches the
operator. The `|` twin reports the same way: a pipeline stage must begin, so
`if true; then echo a |; fi` is
`expected command after pipe` (rc 1, bash rc 2). A plain `&` behind the body is
a terminator, not an operator, and is bash parity (`if true; then echo a & fi`
runs `a` in the background).

A closing keyword closes **its own kind of block**: `fi` an `if`, `done` a
`while`/`until`/`for`, `esac` a `case`, `}` a group. A closing keyword word that
stands in **command position** with no block of its kind open closes nothing, and
bash's grammar has no command that is a lone `fi`/`done`/`esac`: it refuses the
whole line at rc 2 before running anything, and fdshell answers the same way at
its house rc 1 — `stray 'fi' — it closes no open block` with a suggestion naming
the block that closer serves, the caret on the word, and **nothing run** (the
preflight of `script.rs`, the same shape as the async-`&` and stray-`;;` arms; a
line carrying both answers with the earlier arm's message).
`while :; do echo if; fi; echo x; break; done; echo after` is such a line: the
`if` is `echo`'s argument, so the `fi` behind the `;` is in command position and
reports, where bash prints nothing either (the command position is the rule, and
the rows it covers are the *sample* pinned by
`safe/fdshell/tests/cond_blocks.rs::a_stray_closer_in_command_position_reports`,
not its extent). A closer-shaped **argument** word stays word data — bash runs
those lines at rc 0 — so `while :; do echo a; esac; …` and a `done` of a `case`
**pattern** list keep their bodies:
`while :; do case x in done) echo p;; esac; break; done; echo after` runs at
rc 0, as bash does. The **brace** closer is the exception by rule: bash's grammar
has a rule for a lone `}` and the group scanner owns it, so a `}` is never asked
this decision (the divergence rows of `tests/group.rs` stand). A `time [-p]`
prefix written in front of the closer is transparent: the report names the closer
behind it. When no closer of the block's **own** kind is left, the line is the
"unclosed mid-run construct" report of "Errors":
`while :; do echo a; fi; echo after` is
`'while' is missing its closing 'done'` (rc 1, bash rc 2). A body whose **own**
closer ends it above the `break` runs **forever**, and bash hangs on those
shapes too (`while :; do echo while; done; echo after`); where such a line keeps
a second closer of the block's kind, bash escapes at rc 2 — and so does fdshell,
at rc 1, since the stray report covers it (the bash-parity hangs are pinned
separately in `safe/fdshell/tests/cond_blocks.rs`).

A `case` **arm may be empty**: bash reads an arm body as a command list, and an
empty list is a list, so an arm closed straight at `esac` is a complete clause —
`case x in a) esac` runs at rc 0, and so does an empty **last** clause
(`case x in a) echo one;; b) esac`). This is the opposite of the `then`/`do`
bodies above, which bash requires non-empty. Clauses are separated by `;;`, and
a clause written without it is a parse error: the next clause's pattern paren is
a token the arm's command list cannot take, so `case x in a) b) ;; esac`,
`case x in a) *) esac` and `case x in a) echo hi; b) ;; esac` run **nothing** at
rc 1 with `case: missing ';;' between clauses` (bash stops at rc 2). A `)` that
closes nothing the arm opened is judged (task **#419**): only a word-list place
passes, and a depth-0 paren behind an argument (`a) echo hi) ;;`,
`a) echo a[b) ;;`, `a) echo (b) ;;`), behind an operator token
(`a) echo hi; ) ;;`) or with no word before it at all (`a) ) ;;`) is a paren the
command list cannot take — rc 1, bash rc 2. What stays arm text, as it always
was — measured to parse at rc 0: a quoted `")"` and
an escaped `\)` are word text, `a) ((1)) ;;` and an arithmetic
`for ((i=0;i<2;i++)); do …; done` clause are arithmetic data (the `((` word owns
**two** closers), and the paren of a
group **the arm itself opened** closes that group: a function definition
(`case x in x) f() { echo y; }; f ;; esac` prints `y` in bash and here, rc 0,
and the same holds for a definition behind a `;` or inside the arm's
`if`/`while`/`for`/`{ …; }` bodies) and a subshell the arm wrote (`a) (cat) ;;`,
`a) echo hi | (cat) ;;`, `a) g() ( : ) ;;`). An **array-literal assignment**
carries its group's `(` **mid-word** — the tokenizer's word for `m=(1)` is `m=(1`
— so `case x in a) m=(1) ;; esac`, `a) x=(1) y=2 ;;` and `a) arr=("$@") ;;` parse
at rc 0 as bash does; the parse is the gate here, and since task #422 assembles
the literal at the token level the run is an assignment too —
`case x in x) m=(1); echo done ;; esac` prints `done` at rc 0 with clean stderr
(bash). The paren test reads a word's
bytes **with its quote mask** and asks the counter only in **command position**,
so a quoted `(` (`a) echo "x(" b) ;;`) and an
escaped one (`a) echo x\() ;;`, where the mask marks the pair unquoted and only
the backslash says it is escaped) open no group, and the `)` written behind them
is then the arm's stray paren — rc 1. Shapes that stay open: a subshell arm that
**runs** (task #316). A closer glued behind a pattern's `)` is the block's closing
keyword and is read the way bash reads it (the closing-keyword rule, the glued
`esac>f` included): `case x in a)esac`, `case x in a) echo hi;;b)esac` and the
last clause closed straight at `esac` all run at rc 0, and the block-redirect tail
`case x in a) echo hi;;b)esac>f` files nothing in `f` at rc 0 too (task **#421**).
A closer-shaped **pattern** word stays word data (`case x in done) echo p;; esac`
matches the word `done`, task #306), and `case x in esac)esac` keeps its measured
answer — fdshell rc 0 where bash stops at rc 2. Pinned in
`safe/fdshell/tests/case.rs`.

bash's extended clause terminators are not open any more (task **#420**): `;;` stops,
`;&` runs the next clause's body, `;;&` tests the next clause's patterns against the
same word. The tokenizer emits a token for `;` alone and `&` is a word byte, so `;&` and
`;;&` arrive as two and three tokens and only a **glued** pair is the terminator — `; &`,
`; ;&` and `;; &` stay body data, and a plain `&` written in front of a glued terminator
stays the job marker of tasks #354/#355. A `;&`-forced clause never tests — so never
expands — its patterns; a `;;&` retest does. `exit`, `set -e` and loop control raised in
any body of the chain propagate whole, and the status of the last run body owns the
result. The pattern-list rows stay parse errors of their own position (task #524):
`case a in a;& b) echo B;;` and `case x in 1 ;& ) echo hit;;` answer
`case: empty pattern` (rc 1, bash rc 2). Two rows stay divergent by rule, pinned as
measured: `;&esac` (a non-keyword word keeps its whole word, so the block never closes)
and bash's `;;` + newline + `&` — the newline is the list separator, and a
terminator's head is a real `;` byte. Pinned in `safe/fdshell/tests/case.rs` and
`safe/fdshell/src/parse/case_clause/terminator/tests.rs`.

The `;;` is the clause separator **and only that**: POSIX #2.10 puts it inside a
clause and nowhere else, so a `;;` written in any other command list is a parse
error — bash names the token and stops at rc 2, fdshell answers
`stray ';;' — it only separates 'case' clauses` at its house rc 1 and prints
nothing. The rule is **one predicate read at two granularities** (LESSONS, the
task **#355** entry), because the two halves of a command list are seen by
different layers. The **byte** reader owns everything the statement split shows the
scanner — the top-level list, a pipeline, a `&&`/`||` chain: `echo a ;; echo b`,
the glued `echo a;;echo b`, the spaced `echo a ; ; echo b`, a pair opening a
statement (`;; echo b`, `echo a; ;; echo b`) or closing one (`echo a ;;`). A `;;`
*is* a separator there, so the report is a **preflight** over the whole scan and
nothing on the line runs — `echo z; echo a ;; echo b` prints nothing, which is
bash's answer too; a statement run from an **earlier line** of a multi-line input
stays run in bash (`echo z` + newline + `echo a ;; echo b` prints `z`) and stays
unrun here, exactly as the invalid `&`-split preflight of tasks #354/#355 behaves.

The **token** reader owns the lists the scanner never re-splits: a block body, a
group or a function body, and an opener's condition slot — `{ : ;; }`,
`f() { : ;; }; f`, `for i in a; do : ;; done`, `while false; do echo a ;; echo b;
done`, `if : ;; then :; fi`. A body that never **runs** is guarded all the same
(bash calls that row a parse error too); what stays lazy is an inner construct
*inside* an unrun body, the pinned family of
`tests/block_closers.rs::a_never_run_body_is_never_parsed`. The guard answers
**after** the non-empty ensures and only outside a slice's **leading** separator
run, so those spellings keep their own messages — `'then' body is empty`
(`if true; then ;; fi`, and the same pair written as two lines), `group is empty`
(`{ ;; }`), `function definition body is empty` (`f() { ;; }`),
`missing condition` (`if ;; then :; fi`), `malformed if block` (`elif ;; then`),
`expected condition` (`while ;; do :; done`) — while a leading pair with content
behind it runs at rc 0 (`if true; then ;; echo a; fi` prints `a`, `{ ;; :; }` and
`bar() { ;; :; }; bar` print nothing; bash rc 2 on all three). Its **one**
exception is the pair written immediately before a closing keyword, where the
closer-claim above has already claimed the token in front of it:
`if true; then echo a ;; fi` and its spaced twin `if true; then echo a ; ; fi`
print `a` at rc 0, and the body that survives the claim still parses
(`if true; then b ;; fi` ends at its runtime miss `"b" not found`) — pinned as
measured, bash rejecting each at rc 2. What the rule does **not** reach stays as
measured too: a quoted `";;"`/`";"` and an escaped `\;\;` are word data (tasks
#247/#336); a here-doc body, a `$( … )` substitution and an arithmetic
`for (( … ))` region stay opaque; a `;\n;` pair is task **#487**'s — the gap
between the two `;` is spaces and tabs only, at both granularities; `;&`/`;;&`
written **outside** a clause keep the split's own message
(`unexpected character in this context`, bash names the token and stops at rc 2),
and so does a line carrying both shapes, since the preflight order is pinned; and
the `case` clause's own terminator — all three kinds of it, single- and
multi-line, in a body and in an arm — stays legal. Pinned in
`safe/fdshell/tests/semi_semi.rs`, `safe/fdshell/tests/case.rs` and
`safe/fdshell/src/parse/semi/semisemi/tests.rs`.

A `;;` written inside a block **nested in an arm** belongs to that block: the
clause walk's `;;` search accepts a window only at the arm's **own level** — the
same walk the closing-keyword search runs (the block's seed, the `case` word-list
rule, LESSONS "One clause rule, three scorers"), stopped by a different predicate
— so `case a in a) case b in b) echo y;; esac;; esac` and
`case a in a) case b in b) echo y;; esac;; c) echo outer;; esac` print `y` at
rc 0 as bash does, and so does the nested `case` reached through a `while`
(`case a in a) while :; do case b in b) echo y;; esac; break; done ;; esac`) and
the heredoc-carrying nested arm — there the pair **is** a clause's own separator,
which the rule above leaves legal. A `;;` written in the nested block's **body** is
not, so that rule's token reader answers: the nested group
(`case a in a) { echo y;; } ;; esac`), the nested definer
(`case a in a) f() { echo y;; }; f ;; esac`) and a nested `for`/`while` body all
report at rc 1 and print nothing, bash rc 2. The nested `if` row
(`case a in a) if true; then echo y;; fi ;; esac`) still prints `y` at rc 0, and
only through the closer-claim exception above. The pairing counts keyword openers
and exact brace words only, so a `;;` inside a **subshell** in an arm is still the
arm's (`case a in a) (echo y;; ) ;; esac` keeps its verdict), and the nested
`case`'s first pattern `)` stays word-list data behind `in`. One row stays
divergent by rule, pinned: a nested clause list with a **second** clause
(`case a in a) case b in x) echo n;; *) echo inner;; esac;; esac`) — the
stray-`)` rule above reads the nested clause's pattern paren and answers at rc 1,
bash rc 0. Pinned in
`safe/fdshell/tests/case.rs::a_nested_block_in_an_arm_keeps_its_semi_semi` (the
walk's rows) and `…::a_semi_inside_a_nested_arm_block_stays_as_measured` (the two
bodies that now report, the `if` row that does not).

An **empty clause list** is legal only in the spellings bash allows: its slot is
POSIX #2.9.1's `newline_list`, so `case x in esac`, `case x in` + newline `esac`
and the blank-line spelling run at rc 0 in both shells, while a real `;` **byte**
in that slot — `case x in; esac`, `case x in ; esac`, `case x in;; esac`,
`case x in ;;; esac`, and the same `;` written as a newline — is a syntax error
there (bash rc 2, here rc 1 with `case: ';' before an empty clause list`). The
test is on the token's **byte**, so the newline the tokenizer spells as a `;`
token stays legal (task #355's rule); and the guard covers only the **empty**
slot, so the walk's own verdicts (`case: empty pattern`, `case: missing ')'`) and
the leading-`;`-before-a-clause leniency of task #300 stay as they are. Pinned in
`safe/fdshell/tests/case.rs`.

`for ((init; cond; step))` is bash's arithmetic loop: the three clauses are
evaluated exactly like an `((expr))` command — in the current shell, so the
assignments they make survive the loop and `$i` expands in the body (and after
it). The init runs once, the condition before every body, the step after it; a
clause written empty is a no-op and an **empty condition is true**, so
`for ((;;))` runs until its body `break`s out. The loop's status is the body's
last command, and `0` when the body never ran. `do` is required: the region is
followed by at most one real `;`, then any run of newlines, then the keyword
`do` (`)) do`, `)) ; do`, `))`+ newline, `))`+ two newlines, `)) ;`+ newline,
and the glued `))do`), and the body is the usual `do … done` block — a
here-doc inside it is re-read once per iteration. The region is read as raw
bytes, so `;`, `|`, `<<`, a quote and a newline inside it are data:
`for ((i=0;i<$(echo 3);i++))` has three clauses, and the `<<` of `1<<2` claims no
here-doc; a closed `${…}` region in a clause is expanded before the clauses are
cut, as on every arithmetic surface (task #397, see "Arithmetic" above). A word
whose bytes are exactly a keyword word **ends** at an unquoted
metacharacter — one byte set, one reserved list, read at both granularities —
so the glued `for((i=0;i<3;i++))` runs the loop as bash does (task #396).
Behind the region stands `do` — or the `{` of a **group body** (task #394:
bash's `for_action` ends with a compound list), so
`for ((i=0;i<2;i++)) { echo $i; }` runs the group as the loop's body. The group's own
rules are then the body's: its `;`-before-`}` terminator, its emptiness rule, its
trailing redirections, and the gap the tail rule allows before the opener — one
real `;`, then any run of newlines (`)) ; { …; }` and `)); { …; }` both run, and
so do `)) ;`+ newline + `{ …; }` and `))`+ two newlines + `{ …; }`). Shapes that stay open: a
word that is **not** a keyword word keeps its whole word — bash's
general break, the open work of tasks #316/#470/#422/#499. A step spaced as `i ++` is **parity**, not an open shape (task
#395): whitespace never separates an operand from its `++`/`--`, so
`for ((i = 0 ; i < 2 ; i ++)) { echo $i; }` ends like bash's, pinned in
`tests/loops.rs::c_style_for_a_spaced_step_matches_bash`. A clause count that is
not three (`for ((i=0))`, `for ((i=0;i<2;i++;))`) and any word between `))` and
`do` (`))x`, `)) 2>/dev/null`, `)) ;;`) are parse errors, as they are in bash (rc
2 there, rc 1 here) — so is a gap that breaks the rule: a second `;` (`; ;`), a
`;` written **after** a newline, and the `&` family (`&`, `;&`). A pipe behind
the body, the loop behind a pipe, a here-doc in the body and a trailing `&` are the
**group layer's** pinned divergences for those positions (#186/#190, #188/#192), which
the brace body inherits unchanged: bash runs them, fdshell reports the group's error.

`break` and `continue` accept an optional loop count (`break 2`), and are builtins
whose own status is `0`: right after a loop that ends through one of them, `$?` is
`0`, not the status of the last command that ran. The status of a command is `0`
for success and non-zero for failure, so `&&` runs the next command only on success
and `||` only on failure:

```shell
make && echo built || echo "build failed"
for x in 1 2 3; do false; break; done; echo $?   # 0
```

`set -e` treats these lists as exemptions: which failing command ends the shell
is spelled out in "Pipelines and lists" below.

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

A function is a definition, and `unset NAME` reaches it only when no variable of
that name exists (the priority is spelled out in "Variables" above). Definitions
are global, so a name removed by an `unset` inside a function stays gone after the
call returns; a `local NAME` in that same frame is a variable, so the function
survives that one.

A group's closing `}` obeys the closer rule of "Conditionals and loops": a
statement the group opened must be terminated before it — `{ echo a }` is the
parse error `expected ';' before '}'`, `{ echo a; }` runs.

A function's body obeys the block-body rule, the same one the arms and the
group follow: it must be a non-empty list, so `f() { }`, `f() {; }`,
`f() { ;; }`, the comment-only `f() { # c` + `}` and the multi-line `f() {` +
`}` are parse errors — `function definition body is empty`, rc 1 where bash is
rc 2 — and the definition is never made, so `f() { }; f` and `f() { }; echo
tail` run nothing; the `function f { … }` spelling follows the same rule. A
body of only the plain `&` is **not** an empty body, and a definition's body
**text** is judged when the function runs, so `f() { & }` defines at rc 0 and
its call is the parse error (bash rejects the definition outright, rc 2 — the
statement-by-statement parse rule,
[POSIX_COMPATIBILITY.md](POSIX_COMPATIBILITY.md#quoted-keywords-are-command-words-quoted-control-keywords-are-not)).

A definition ends at its `}`, and a **word** written behind it is fdshell's
accepted define-and-call idiom: `function f { echo hi; } f` runs the body at rc 0
and `f() { echo hi; } $( echo x )` runs the trailing word at rc 1, where POSIX and
bash want `;`, a newline or the end of input and reject both with a syntax error at
rc 2. The `;`-twin control `f() { :; }; echo ok` runs in both shells at rc 0; the
family is pinned in
`tests/function_keyword.rs::a_definition_behind_a_trailing_word_runs_it` (task
#368).

POSIX and bash put the `{` right behind the definer words, so a **redirection**
written between them is a syntax error (bash rc 2, naming the token it trips on), and
fdshell reports it at rc 1: `f() <<E { :; }`, `f() > /dev/null { :; }`,
`f() >/dev/null { :; }`, `f() <<<x { :; }`, `f() 2>&1 { :; }`, `f() &>f { :; }` and
the io-number `f() 3<<T { :; }`, each in both definer spellings, are one report each
— `'<word>' cannot be written between the function definer and its '{'` — with no
definition made and nothing else in the script run. The **trailing** form
`f(){ …; } > a` stays the accepted spelling (task #180); a quoted operator word
stays word data (task #247) and a plain filler word in front of the `{` stays the
documented leniency (`function f x { …; }`). The family is pinned in
`tests/function_keyword.rs::a_redirection_before_the_opening_brace_is_a_parse_error`
(task #433).

### Pipelines and lists

`cmd1 | cmd2` connects cmd1's stdout to cmd2's stdin. `;` separates
statements, `&&`/`||` gate them, and a trailing `&` turns the statement before
it into a background job (fdshell also offers the named form `cmd &>&name`,
above). `time cmd` reports how long cmd
took; `timeout seconds cmd` stops it after a deadline. A list takes **one** `;`:
the doubled `;;` is the `case` clause separator and nothing else, so a stray one
anywhere in a list is a parse error — `echo a ;; echo b`, its glued
`echo a;;echo b` and its spaced `echo a ; ; echo b` are one report each, not two
statements (bash rc 2, fdshell rc 1, and nothing on the line runs — the preflight's
shape; the full rule, its two readers and its boundaries are in "Conditionals and
loops" above).

`set -e` (`set -o errexit`) ends the shell at a failing command, with that
command's own status. Two positions are exempt, as in bash: a command run in an
`if`/`elif`/`while`/`until` **condition**, and any command that is part of a
`&&`/`||` list except the command following the final operator. The list
exemption reaches **everything a compound in that position executes** — a
`for`/`while`/`until` body, a `{ …; }` group, an `if` then/elif/else body, a
`case` arm, a function frame, a `source`d file — so
`set -e; { false; } && echo after; echo rc=$?` prints `rc=1` and ends 0, and
`set -e; for x in 1; do false; done && echo after` runs on. The exemption
suppresses the exit, never the status: the list ends with the failing status,
and it ends with the list, so the next statement's failure is fatal again. A
compound that **is** the command following the final operator keeps exiting —
`set -e; true && { false; }; echo rc=$?` exits 1. bash's third exemption, every
pipeline stage but the last, is not implemented: a failing command in a
non-final stage ends the shell (task #490). A redirection whose target cannot be
opened aborts the whole script here, `set -e` written or not, where bash fails
only that command (task #179).

A `&&`/`||` with **no command behind it** reports, at statement level too:
`echo a &&`, `echo a && #c` and `echo a && ; echo b` print nothing at fdshell's
rc 1 where bash stops at rc 2 (task #367). The report fires **before** any part
of the list runs. Bash parses its whole input before running any of it, while
fdshell's report reaches one scan pass, so a statement that already ran stays
run: `echo z; echo a &&` prints `z` and stops at rc 1. The interactive shell
keeps buffering such a line, as "Interactive shell" below says, because the
construct is not complete; at the end of input it runs the buffer, and the
report fires there. A `&&`/`||` at the end of a physical line **does** continue
the list onto the next line, as
POSIX #2.9's AND-IF/OR-IF terminator says: the short circuit, the gate and the
list's status all span the joined lines, so `false &&` + newline + `echo b` runs
nothing and ends 1, and `true ||` + newline + `echo b` runs nothing and ends 0.
Glued (`false&&`), trailing-blank (` &&`) and leading-blank (` echo b`) spellings
join the same way; a quoted `"&&"` or an escaped `\&&` stays a word, so nothing
joins there (task #496), and `;` or the end of input ends the list (task #367). A `#`
comment tail is not part of the predicate's text, so `false && #c` + newline + `echo b`
joins too and reaches bash's answer; a comment written on one of the **middle** lines of a
joined chain keeps its own newline (the parse layer reads comment text only up to it), so the
chain splits there and `cat <<B &&` + `body` + `B` + `echo c1 && #c` + `echo c2 &&` +
`echo last` prints `body`, `c1` and `c2 && echo last` where bash prints `body`, `c1`, `c2`
and `last`. The
`set -e` exemption of the paragraph above spans the joined lines too, so
`set -e; false &&` + newline + `echo b` + newline + `echo tail` prints `tail` and
ends 0.

The rule reads **both** sides of the operator, so one that **opens** a physical line
reports: in `echo a` + newline + `&& echo y` nothing sits in front of the `&&`, and bash
rejects the line at rc 2 while fdshell reports `expected command before '&&'` / `'||'` at
the house rc 1. The report fires **before** any part of the list runs and what already ran
stays run, so that row prints `a` and ends 1 — the same shape written as the first line of a
block body, a loop body, a `case` arm, a function body or a `{ …; }` group, and through the
interactive shell, where the session reads on to its own end-of-input answer. The walk covers
a text that **begins at a physical-line start**, so an operator opening a part inside such a
text reports too (`echo a && && echo b`, `echo a &&` + newline + `&& echo b`, `false || &&
builtin true`). What stays outside is a text that begins **mid-line** — the tail of a line
whose head another mechanism claimed: a definition's body (`f() { echo hi; } && echo done`
runs in both shells at rc 0) or a here-doc region — and a text that is **only** an operator,
where both sides are blank and each rule declines it. A `#` comment on the line above does
not shield the operator either: the parse layer reads comment text only up to its newline, so
`if :; then` + `#c` + `&& echo y` + `fi` reports too (task #406).

A stage is a **command**, so the word behind a `|` starts a command and a keyword
block can be a stage: `echo a | while read l; do echo "$l"; done` runs the loop in
the stage's own fork with the upstream stage's output on its stdin, and
`echo a | if :; then echo y; fi` the `if`. The block ends at its own closing
keyword, and what is written behind it (`; echo tail`, `&& echo y`) is the next
statement; a redirection written there belongs to the block, so
`echo a | while :; do echo b; break; done > f` files the stage's output (see
"Redirection" above). A `name() { … }` definition in a stage dies with the fork.
Two shapes deviate: `time` behind the `|` is the stage's command name, not a
prefix of the block's keyword (bash rejects that line outright, rc 2); and a
`{ …; }` written as a stage stays a word, so the line is a parse error where
bash runs the group in the stage's fork (`echo a | { echo b; }` prints `b`, rc 0
— the group rule of tasks #186/#190, see "Functions and groups" above). A `|`
at the end of a line stays a hard parse error: a stage must begin on the line
that opens the pipe, so `echo a |` + `while :; do :; done` is rejected where
bash runs the loop.

A stage's **name** resolves through the same order a plain command uses: a user
function first, then the builtin tables, then `PATH`. So `f(){ echo hi; }; f | cat`
prints `hi` at rc 0, and `f(){ echo a; }; g(){ cat; }; f | g` runs a function in
each of the two forks. The stage's fork is a **subshell**, so what a function
changes there dies with it: a `cd`, an assignment or an `exit` written in the body
leaves the parent's working directory, variables and life alone — bash's stage
subshell behaves the same, and `f(){ exit 3; }; cat | f; echo rc=$?` prints `rc=3`
in both. The `builtin`/`command` prefixes bypass functions (POSIX #2.4), and so do
the handlers a plain command runs in the parent before forking — `cd`, `read`,
`set --`, `wait`, `eval`, `alias`, `export`, `local`, `exit`, … — which a stage runs
in its own fork (task #381): what they change dies with the fork there too, so
`cd sub | cat; pwd` prints the directory the stage started in,
`export E=1; echo a | export E=2; echo "$E"` prints `1`, and a stage's `exit 3` is
the pipeline's code only when the stage is its last one. What stays outside a stage
is what has no builtin at all — the parse-level commands of "Builtins" above — and
the forked table answers `builtin` first, so `echo a | builtin cd sub` reports
`"cd" is not a shell builtin` at rc 1.

A `#` at the start of a word comments out the rest of its **physical** line, so
no `&&`/`||` written inside it splits a list: the rule reaches a keyword block's
closing keyword (`while :; do echo a; break; done # c && echo y` prints `a`
alone, and the mid-run block `true && while :; do :; break; done # c && echo y`
prints nothing), and the gate of a short-circuited `||` ends at the comment's
newline — `true || if :; then :; fi # c ; echo tail` never resumes inside the
comment and runs nothing. The rule reaches a here-doc operator on a
comment-terminated line too: `cat <<Q # c` + `body` + `Q` prints `body`, because
the comment's newline is where the body starts and the line keeps its own region.
A `#` inside a word (`echo a#b`) is data.

### Aliases

```shell
alias ll='ls -l'
unalias ll
```

### Interactive shell

Run with no arguments, fdshell prompts with `fdshell> ` and reads one logical
input at a time; a construct that is not complete — an open block, an
unbalanced quote, a dangling `&&`/`||`, or a here-document whose body has not
arrived — continues under the `> ` prompt until it closes. A here-document's
body lines are read as continuation lines, **including inside a closed
single-line keyword block**: `for x in a; do cat <<T; done` + `{a,b}` + `T`
buffers the two body lines and prints `{a,b}`, like bash. A group body's
here-document (`{ cat <<T; }`) buffers its body lines the same way and the
buffer runs with the body attached, printing `{a,b}` — the prompt trail and the
output are byte-ident to the keyword-block rows.

A construct opened **mid-run** — `true && { …`, `true && while :; do …`,
`echo a | while :; do …`, `true && f() {` — is incomplete the same way and
buffers under `> ` until its closer arrives: the scan skips such a span inside
its run, so it is the scan's record of the missing closer, never an open block,
that keeps the prompt reading. `true && while :; do` + `echo hi` + `break` +
`done` buffers the three continuation lines and runs as **one** statement,
printing `hi`. A run-start group whose **own** brace closed but whose tail group
did not buffers too — `{ echo a; } && { echo b` (task #401) — because what the
prompt reads is the **statement's** closedness, not the construct's own. The
divergence is what happens at end of input: fdshell executes the buffer it has,
so `echo a; true && { echo g` and `{ echo a; } && { echo b` print `a` and report
`group is missing a closing brace` (rc 1 in a script), where bash parses the
whole line first, runs nothing, and exits 2 — the REPL's rc-1-vs-2 family. A
brace at the very end of input **in command position** joins this family since
task #402 — the byte rule counts the missing byte as an opener boundary, so the
run-start `{`, the mid-run `true && {` and the arithmetic `for`'s
`for ((…)) {` buffer, and the EOF answer is the family's own report (the scan
level's missing-closer for the run-start shapes, the parse layer's for the
mid-run one, bash naming `{` at rc 2). What stays answered at once is the
glued/brace-expansion set (`{echo`, `{;`, `{a,b}`, `{}`) and an
**argument-position** brace (`echo {` prints `{`) — the callers'
command-position rule keeps those words, in bash and here.

End of input is the `ignoreeof` policy (`set -o ignoreeof`, `set -i`, on by
default when stdin is a terminal): on a **terminal** an EOF prints
``use `exit' to leave`` and the shell keeps reading, so a stray Ctrl+D is
harmless; on a **pipe or file** — script input — the same EOF prints the hint
once per read and the shell exits 0, because a pipe's EOF repeats and
ignoring it would print the hint forever. bash exits 0 silently on that same
pipe (its non-interactive `set -i` is a `set: -i: invalid option` error, so a
bash script never reaches the policy); fdshell keeps the hint, the only
observable that shows the option took effect.

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
must stay relative — the check runs on the **expanded** word, so a target
expanding to `/etc/x` is refused as well, and so is a `hash` pin whose path
only the expansion makes absolute (`hash fdprobe $PWD/target`). `exec`/`become` inherit the
ban. `cd ..` remains the user's own escape hatch: the base follows the shell,
exactly as a relative path does. `cd`'s operand is not covered by the ban, on
the raw word or on the expanded value (`shopt -s strict; d=/tmp; cd $d` moves
the base): the ban list names redirects, program words and `hash` pins, and
moving the base is what `cd` is for.

`strict` is inherited by the forked command-substitution child, so the program
word and `PATH` bans apply inside `$( … )` and `` ` … ` `` as well: with an
ambient absolute `PATH`, fdshell's own builtins (`builtin …`) are the only
programs a substitution can run, and an external program runs only through a
**relative** `PATH` entry resolved at the pinned `%CWD` base. A program the
child cannot run is reported by the **child**, on stderr — its stdout *is* the
capture pipe — and contributes no bytes, so the expansion is empty: an argument
word then carries no field (the command runs without it, rc 0 — bash's shape),
and a redirect target lands on 0 fields, which is bash's `ambiguous redirect`:
the message names the word, not the cause, because the parent can only see
empty output. Pinned by
`tests/strict.rs::strict_substitution_needs_a_relative_path_for_an_external_program`
and `tests/cmd_subst.rs::strict_external_in_a_substitution_yields_an_empty_field`.

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
is AF_INET v4, and `--port 0` asks the kernel for a free ephemeral port.
Backgrounding an `accept` (`builtin accept %l %>%c &>&t;
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

* Data-driven recursion — the arithmetic evaluator/parser, `test` expressions
  and brace expansion — is capped at 1 000 levels of nesting. bash computes
  deeper input; fdshell reports a clean error instead of aborting on a stack
  overflow.

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
