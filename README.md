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
and `break &` leave the parent untouched. `&` is only the operator at a
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

`for`'s word list stays strict — `for i in 1 & do :; done` is a parse error —
and a `&` behind a `&`, or a `;;` behind a `&` outside a block (`echo a & ;;
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

The `&>` family's fd-2 half is applied in fd 1's slot of the command's
redirection table, so a `2>`-targeted redirection written **after** `&>` wins
fd 2: `cmd 2>&- &> f` fails (rc 1, fd 2 closed) and `cmd 2>&1 &> f` sends fd 2
to the pre-redirect fd 1, where bash applies `&>` later and reopens fd 2 onto
`f`. (Tracked by task #337; the `&>` bytes for fd 1 and for a plain `&> f`
match bash.)

The target of `>` can also be `%var` (an fd variable), `/dev/fd/N` (a copy of
the open fd `N`), `>&1` (a copy of fd 1), `2>&%f` (a copy of the fd `%f`
refers to) or `2>&-` (close fd 2):

```shell
builtin openat2 --dirfd %CWD --flags O_CREAT --flags O_WRONLY --mode 0644 log %>%log
echo "written through the variable" >%log
```

An operand that begins with `<`, `>`, `&` or `%` is a file name when that
byte is quoted — `echo hi > "&f"` opens `&f`, as bash does — and a syntax
error when it is not (`> &f`); the spaced `%var` operand stays a syntax
error, the fd-variable targets being the attached `>%var` / `&>%var` forms
([POSIX_COMPATIBILITY.md](POSIX_COMPATIBILITY.md#operators-are-recognized-at-word-starts)).

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

The fd target forms are decided at parse time — the attached `>%var`,
`/dev/fd/N`, `>&N` — so a `%` byte inside a target word is file-name data:
`> a%b` creates `a%b`. The `>&N` duplicate word is not a word to expand
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
there is no command for the prefix to scope to. Two side effects of that
in-process run are accepted divergences: the prefix is also **exported** (bash
keeps such an assignment shell-local), and a `set -x` trace prints nothing for
the statement (bash prints a line for the command it traces). A word behind the
redirections is the command word — `>f g` runs `g` with the redirection — and a
quoted operator word (`'>f'`, `"&>f"`) is a command name, never a redirection;
a statement carrying a here-doc body keeps the behavior its own pins record.

A **keyword block** takes a redirection written after its closing keyword and
applies it to the whole block: `while :; do echo a; break; done > f` files the
loop's output in `f`, and so does `until … done > f`, `if … fi > f`,
`for … done > f`, `for ((…)); do …; done > f` and `case … esac > f` — the glued
spelling `done>f`, `fi>f`, `esac>f` included. The scope opens **before** the
block's own word list or condition runs, so a target that fails to open stops
the block before its first command expands anything (`for x in $(touch m; echo
a); do :; done > /nope/x` leaves no `m`; the rc-1 abort is task #179's), and it
is undone at the block's end, so an `exec > z` inside the body dies with the
block. The tail follows the simple-command rules: several redirections apply in
order and the last one on a descriptor wins (`done > f > g` leaves `f` empty and
`g` the output), a word glued to an operator is one word — `done>f>g` opens the
file `f>g`, fdshell never splitting a word at an operator byte, as above. What
the tail deliberately does **not** read is a plain word behind the closer
(`done x`, task #405), a here-doc or here-string on it (`done <<T`, `done
<<<hi`, task #408) and the `&>` family there (`done &> f`, task #409): each
keeps the behavior its own pins record.

### Here-documents

A here-document feeds a command's stdin from the lines that follow it, up to
a delimiter line:

```shell
cat <<EOF
line one
line two
EOF
```

An unquoted delimiter expands `$`/`$(…)`/backticks in the body. A **quoted**
delimiter makes the body literal, and any quote operator in the delimiter word
quotes it — `<<'EOF'`, `<<"EOF"`, `<<E\OF`, `<<EO\F`, `<<\E\O\F`, `<<\$HOME`
(POSIX XCU 2.3 counts `\` a quote operator, so the escape-only word `<<E\OF`
quotes the body exactly like `<<"EOF"`). The word is folded for matching, so
`<<E\OF` delimits `EOF`, and a quoted body is verbatim: nothing expands in it
and no `\<newline>` continuation is stripped from it. `<<-EOF` strips leading
tabs from the body and the delimiter. A here-document is usually attached to
`cat`, `read` or another command that reads stdin. A bare delimiter word
that begins with `<`, `>`, `&` or `%` is a delimiter when that byte is
quoted (`cat << "<x"` + body + `<x` prints the body, as bash); unquoted it
is a syntax error.

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
is a separate statement.

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

The stream is read **greedily, one body per operator**, as bash reads it: each
operator's body starts where the previous operator's delimiter line ended, and
an operator whose delimiter line never arrives takes what is left of the stream
as its body (empty when nothing is left) — the operators behind it get an empty
body, and the body lines never run as commands. So `cat <<T; cat <<E` +
`bodyT`/`T`/`extra` prints `bodyT` and `extra` (rc 0), and
`if true; then cat <<T; elif cat <<E; then echo b; fi` + `bodyT`/`T` prints
`bodyT` (rc 0). What stays a limitation is the **first** operator's missing
delimiter line: `cat <<E` + `body` reports
`here-doc: missing terminating delimiter line 'E'` (rc 1), where bash warns
`here-document … delimited by end-of-file` and runs.

A number in front of the operator puts the body on that descriptor instead of
stdin: `cat 3<<EOF` makes the body fd 3's input and leaves stdin alone, and
`exec 3<<EOF` keeps it there for the rest of the script. The number must be
its own unquoted digit word, as in `3>file` — `cat "3"<<EOF` and `cat \3<<EOF`
are the single argument `3<<EOF`, its quotes taken as ordinary word quoting and
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
rest of the script (`exec 3<<<text; cat <&3` prints `text`). The number must be
its own unquoted digit word, as in `3>file` — `cat "3"<<<x`, `cat 3"<<<"x` and
`cat \3<<<x` are the single argument `3<<<x`, its quotes taken as ordinary word
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
opener's `(` and `$( echo true ) { :; }` runs the words instead of opening a
definition. An unclosed block names the keyword it wants —
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
`unset IFS` restores the default split set (space, tab, newline). No operand is
validated: `unset 1abc` names a variable no expansion can reach and exits 0,
which is what bash does too.

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

A **bare** name — `$v`, `$1`, `%fd` — ends at the closing quote of the quote
region that holds the trigger byte, as it does in bash: `"$v"bc` is `v` plus the
literal `bc`, `"$v""b"` is `v` plus `b` (the second `"` opens a new region),
`"$1""2"` is `$1` plus `2`, and `"$v"'x'y` is the value of `v` plus `x` plus `y`.
Inside one region the name stays greedy (`"x$vbc"` is the value of `vbc`), and
the braced forms are unaffected — `${name}` ends at its `}`.

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

A quoted byte never splits and never globs. `set -f` disables pathname
expansion; `shopt -s dotglob` lets patterns match a leading dot;
`shopt -s nullglob` deletes a pattern with no match; `shopt -s failglob`
makes it an error. `[!…]` and `[^…]` negate a bracket expression (bash accepts
both markers; POSIX defines only `!`).

A `case` **pattern** gets neither treatment: it is one word, never split on the
`IFS` and never globbed, and only its **first expanded word** is the pattern —
so a quoted `"$@"` in a pattern is the **first positional** (bash keeps
`es->word->word`), an unquoted `$@` space-joins, and `"$*"` joins with the
first `IFS` byte (concatenating when the `IFS` is empty). Two measured
divergences are pinned: with an empty `IFS` and an empty positional
(`IFS=; set -- a ""`) bash's pattern text carries its quoted-null byte (0x7f)
and matches nothing, where fdshell space-joins; and a braced `${@}` keeps
fdshell's first-`IFS`-byte join, where bash space-joins an unquoted `${@}` and
takes the first positional in a quoted `"${@}"`.

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
`tests/cmd_subst.rs::word0_scoped_ifs_divergence` family); `cd`'s option words
`--`, `-L`, `-P` and `-x` are operands (task #380); `$_` keeps the raw word
(task #376); and `cd -` prints nothing, where bash prints the new directory
(task #378).

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
`cat` with fds 3 and 4 on `f` where bash runs `cat 3` and `cat 4`; the attached
one-non-empty-word form `>{,zz}` is a parse error where bash opens `zz`; and a
failed redirection aborts the **script**, where bash skips only the command.

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

A redirection written after a block's closing keyword belongs to the **whole**
block: `for x in a b; do echo $x; done > f` files the loop's output in `f`, the
glued `done>f` included, and the block's own fd changes are undone when it ends
(see "Redirection" above).

A block's **condition** must be a non-empty list: `if ; then echo x; fi`,
`elif ; then`, `while ; do :; done`, `until ; do :; done` and their `&` and
newline spellings (`if` newline `then`, `if & then`, `while & do`) are parse
errors — bash stops with rc 2, fdshell with rc 1 and its own message
(`missing condition` for `if`/`elif`, `expected condition` for the loops). An
empty condition is *true* at run time, so `while ; do echo x; done` would never
end. The rule covers conditions only: a `for` **word list** may be empty
(`for i in; do echo x; done` iterates zero times, bash rc 0), a `case` word is
not a condition, and an empty `for ((…))` arithmetic condition is true, as in
bash.

A `for` **word list** is read as **words**: no keyword scores inside it, so
`for i in done`, `for i in esac`, `for i in { 1 2` and `for i in }` are ordinary
words and the block runs from its own `do` to its own `done` —
`for i in done; do echo "$i"; done` prints `done`, at rc 0 as in bash. A `case`
**pattern list** is the opposite place: `esac` **terminates** it, so
`case x in esac` is a complete `case`, while `case done in done) echo p;; esac`
is fdshell's parse error (task #310). A list word glued to an operator is word
data here, so `for x in done>f; do echo A; done>f2` files `A` in `f2` and
`for i in a && b; do echo hi; break; done` iterates — both at rc 0 where bash
stops at rc 2 (accepted divergences). A missing `in` stays fdshell's own rc 1:
`for i 1 2` is `expected 'in'`, bash rc 2.

A block's **closing keyword** needs a command-list terminator in front of it —
POSIX #2.9's `;` (a newline is one) or the plain `&` — with the one exception
bash makes: the terminator after a **compound command** is optional, so a body
whose last statement is an `if`, a loop, a `case`, a group, a subshell or a
function definition closes straight at its own keyword. `while false; do if :;
then :; fi done`, `for i in a; do if :; then echo f; fi done`,
`if true; then { echo a; } fi`, `if true; then f() { :; } fi` and
`while false; do ( : ) done` (a subshell's `)` is a closer too; a subshell that
**runs** is task #316's gap), mid-run (`true && if …; fi`) and as a pipeline
stage (`echo a | while …; done`) alike, all run at rc 0. A block keyword written as an
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
a redirection (`while false; do { :; }>f done`). A `;;` written in a `then` body
is the separator rule's, not this one's: `if true; then echo a ;; fi` prints `a`
at rc 0 where bash stops at rc 2.

A closing keyword closes **its own kind of block**: `fi` an `if`, `done` a
`while`/`until`/`for`, `esac` a `case`, `}` a group. A closer of a **foreign**
kind written in a body is an ordinary **command word** — it scores nothing and
does not end the block, so the body keeps the statement written behind it:
`while :; do echo if; fi; echo x; break; done; echo after` prints `if`, `x` and
`after` and exits 0, reporting the stray closer as
`failed to resolve command path: "fi"`, where bash refuses the whole line at
rc 2 (`fi` is a reserved word in its grammar, never a command) — the accepted
acceptance divergence of the stray-closer family. The pairing is also why a
`done` of a `case` **pattern** list stays a list word:
`while :; do case x in done) echo p;; esac; break; done; echo after` runs at
rc 0, as bash does. When no closer of the block's **own** kind is left, the line
is the "unclosed mid-run construct" report of "Errors":
`while :; do echo a; fi; echo after` is
`'while' is missing its closing 'done'` (rc 1, bash rc 2). A body whose **own**
closer ends it above the `break` runs **forever**, and bash hangs on those
shapes too (`while :; do echo while; done; echo after`); where such a line keeps
a second closer of the block's kind, bash escapes at rc 2 and fdshell runs
forever — the same acceptance divergence. Pinned in
`safe/fdshell/tests/cond_blocks.rs`; those rows are a **sample of the rule**,
not its extent.

A `case` **arm may be empty**: bash reads an arm body as a command list, and an
empty list is a list, so an arm closed straight at `esac` is a complete clause —
`case x in a) esac` runs at rc 0, and so does an empty **last** clause
(`case x in a) echo one;; b) esac`). This is the opposite of the `then`/`do`
bodies above, which bash requires non-empty. Clauses are separated by `;;`, and
a clause written without it is a parse error: the next clause's pattern paren is
a token the arm's command list cannot take, so `case x in a) b) ;; esac`,
`case x in a) *) esac` and `case x in a) echo hi; b) ;; esac` run **nothing** at
rc 1 with `case: missing ';;' between clauses` (bash stops at rc 2). A `)` that
closes nothing the arm opened, and that no word in command position precedes,
stays arm text, as it always was — measured to parse at rc 0: a quoted `")"` and
an escaped `\)` are word text, `a) ((1)) ;;` and an arithmetic
`for ((i=0;i<2;i++)); do …; done` clause are arithmetic data, and the paren of a
group **the arm itself opened** closes that group: a function definition
(`case x in x) f() { echo y; }; f ;; esac` prints `y` in bash and here, rc 0,
and the same holds for a definition behind a `;` or inside the arm's
`if`/`while`/`for`/`{ …; }` bodies) and a subshell the arm wrote (`a) (cat) ;;`,
`a) echo hi | (cat) ;;`, `a) g() ( : ) ;;`). An **array-literal assignment**
carries its group's `(` **mid-word** — the tokenizer's word for `m=(1)` is `m=(1`
— so `case x in a) m=(1) ;; esac`, `a) x=(1) y=2 ;;` and `a) arr=("$@") ;;` parse
at rc 0 as bash does; fdshell has no array runtime, so the assignment itself
dispatches the literal's `)` as a command at run time, exactly as it does at top
level (task #422) — the parse is the gate here. The paren test reads a word's
bytes **with its quote mask**, so a quoted `(` (`a) echo "x(" b) ;;`) and an
escaped one (`a) echo x\() ;;`, where the mask marks the pair unquoted and only
the backslash says it is escaped) open no group. Shapes that stay open: a `)`
written after an argument inside an arm (task #419), a nested `case` in an arm
(task #418), bash's `;&`/`;;&` terminators (task #420), a subshell arm that
**runs** (task #316), and a closer glued behind a pattern's `)`
(`case x in a) echo hi;;b)esac`, task #421). Pinned in `safe/fdshell/tests/case.rs`.

`for ((init; cond; step))` is bash's arithmetic loop: the three clauses are
evaluated exactly like an `((expr))` command — in the current shell, so the
assignments they make survive the loop and `$i` expands in the body (and after
it). The init runs once, the condition before every body, the step after it; a
clause written empty is a no-op and an **empty condition is true**, so
`for ((;;))` runs until its body `break`s out. The loop's status is the body's
last command, and `0` when the body never ran. `do` is required: the region is
followed by at most one `;` and the keyword `do` (`)) do`, `)) ; do`, `))`+
newline, and the glued `))do`), and the body is the usual `do … done` block — a
here-doc inside it is re-read once per iteration. The region is read as raw
bytes, so `;`, `|`, `<<`, a quote and a newline inside it are data:
`for ((i=0;i<$(echo 3);i++))` has three clauses, and the `<<` of `1<<2` claims no
here-doc. Shapes that stay open: a `{ …; }` body where bash accepts it
(task #394), the glued `for((i=0;i<3;i++))` with no space after `for` (task #396),
a step spaced as `i ++` (its side effect is lost, so the loop never ends — task
#395), and a `${…}` expansion in a clause, which is a syntax error on every
arithmetic surface (task #397). A clause count that is not three (`for ((i=0))`,
`for ((i=0;i<2;i++;))`) and any word between `))` and `do` (`))x`, `)) 2>/dev/null`,
`)) ;;`) are parse errors, as they are in bash (rc 2 there, rc 1 here).

`break` and `continue` accept an optional loop count (`break 2`), and are builtins
whose own status is `0`: right after a loop that ends through one of them, `$?` is
`0`, not the status of the last command that ran. The status of a command is `0`
for success and non-zero for failure, so `&&` runs the next command only on success
and `||` only on failure:

```shell
make && echo built || echo "build failed"
for x in 1 2 3; do false; break; done; echo $?   # 0
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

### Pipelines and lists

`cmd1 | cmd2` connects cmd1's stdout to cmd2's stdin. `;` separates
statements, `&&`/`||` gate them, and a trailing `&` turns the statement before
it into a background job (fdshell also offers the named form `cmd &>&name`,
above). `time cmd` reports how long cmd
took; `timeout seconds cmd` stops it after a deadline.

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

A `#` at the start of a word comments out the rest of its **physical** line, so
no `&&`/`||` written inside it splits a list: the rule reaches a keyword block's
closing keyword (`while :; do echo a; break; done # c && echo y` prints `a`
alone, and the mid-run block `true && while :; do :; break; done # c && echo y`
prints nothing), and the gate of a short-circuited `||` ends at the comment's
newline — `true || if :; then :; fi # c ; echo tail` never resumes inside the
comment and runs nothing. A `#` inside a word (`echo a#b`) is data.

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
printing `hi`. The divergence is what happens at end of input: fdshell executes
the buffer it has, so `echo a; true && { echo g` prints `a` and reports
`group is missing a closing brace` (rc 1 in a script), where bash parses the
whole line first, runs nothing, and exits 2 — the REPL's rc-1-vs-2 family. A
brace whose next byte is not an opener boundary is a **word**, so a brace
written at the very end of input (`true && {`, and the run-start `{`) is
answered at once: the group-opener word rule's limitation, not the mid-run one.

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
