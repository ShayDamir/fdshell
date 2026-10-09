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
| `openat2 [--dirfd N] [--mode MODE] [--resolve FLAGS] [--flags FLAGS] [--path] path` | Open or create a file via `openat2`. Returns one fd. |
| `mkdirat [--dirfd N] [--mode MODE] [--resolve FLAGS] path` | Create a directory via `mkdirat` + `openat2`. Returns one fd. |
| `mkfifoat [--dirfd N] [--mode MODE] [--resolve FLAGS] path` | Create a fifo via `mkfifoat` + `openat2 O_RDWR`. Returns one fd tagged `fifo`. |
| `pipe [--flags FLAGS]` | Create an anonymous pipe via `pipe2`. Returns two fds tagged `rd` and `wr`. |
| `renameat2 [--olddirfd N] [--newdirfd N] [--flags FLAGS] oldpath newpath` | Rename or exchange files via `renameat2`. Returns no fd. |
| `unlinkat [--dirfd N] [--flags AT_REMOVEDIR] path` | Remove a file, or a directory with `AT_REMOVEDIR`, via `unlinkat`. Returns no fd. |
| `symlinkat [--dirfd N] target linkpath` | Create a symbolic link via `symlinkat`. Returns no fd. |
| `utimensat [--dirfd N] [--atime SPEC] [--mtime SPEC] [--flags AT_SYMLINK_NOFOLLOW] path` | Set a file's atime/mtime via `utimensat`. Returns no fd. |

Flags are named constants (`O_CREAT`, `O_NONBLOCK`, `RENAME_NOREPLACE`, etc.) or
`0x`-prefixed hex values. Repeat `--flags` to combine multiple flags. `unlinkat`
accepts only `AT_REMOVEDIR` (or `0x0`).

`unlinkat --dirfd N` pins the *parent* directory to the fd, but the kernel still
resolves the final path component, so the builtin is not TOCTOU-free: keep the
trailing name short and the `--dirfd` pinned when it matters.

`symlinkat target linkpath` stores `target` verbatim (it is never resolved) and
creates the link entry `linkpath`; `--dirfd N` pins the *parent* directory to the
fd with the same TOCTOU caveat as `unlinkat`.

`utimensat`'s `SPEC` is `now` (the current time), `omit` (leave that timestamp
untouched), or a decimal epoch-seconds integer (negative = pre-1970). An omitted
`--atime`/`--mtime` defaults to `now` (touch semantics). `--flags
AT_SYMLINK_NOFOLLOW` acts on the link itself, not its target.

`openat2 --path` opens with `O_PATH` — a handle with no read/write permission on the file
itself; combine with `statx %fd` to inspect files the user cannot open.

`openat2 --same-as %fd` checks the opened file against the `%fd` reference at open time
(fstat of the just-opened fd, so no path re-lookup between open and check): on mismatch
the opened fd is dropped and the builtin fails, so the capture is never committed.

```shell
builtin openat2 --flags O_RDONLY file %>%ref
builtin openat2 --same-as %ref --flags O_RDONLY file %>%f   # fails if file changed
```

## Strict mode

The `strict` shell option (off by default) turns fdshell into a capability
shell in the Capsicum spirit: the `*at` file builtins ban the two forms of
*absolute path resolution* and require every operation to be relative to an
**explicit `--dirfd`**.

Enable it with `shopt -s strict` (or `set -o strict`); disable it with
`shopt -u strict` (or `set +o strict`):

```shell
shopt -s strict
builtin mkdirat --dirfd %CWD --mode 0755 sub %>%sub
builtin openat2 --dirfd %sub --flags O_CREAT --flags O_EXCL --flags O_RDWR --mode 0644 f %>%f
shopt -u strict
```

While `strict` is on, each of these `*at` builtins — `openat2`, `mkdirat`,
`mkfifoat`, `renameat2`, `unlinkat`, `symlinkat`, `utimensat` — rejects:

- an **omitted `--dirfd`** (or `--dirfd AT_FDCWD`), which would resolve
  against the process CWD, and
- an **absolute path** (leading `/`), which the kernel resolves from the
  filesystem root regardless of `--dirfd`.

`renameat2` pins *both* dirfds and constrains *both* paths. For `symlinkat`,
only the link *location* `linkpath` is constrained; the link *content*
`target` is stored verbatim and never resolved, so an absolute `target` is
still allowed.

Strict mode covers the dirfd-based builtins only. Redirect targets
(`> file`), external command paths, and `~`/`$HOME` expansion are not
dirfd-relative in this version and are tracked as follow-up work.

## Redirection

Operators: `>` (write), `>>` (append), `<` (read), `<>` (read and write),
each with an optional numeric fd prefix (`2> file`, `1< file`, `3<> file`;
no prefix means fd 1 for `>`/`>>` and fd 0 for `<`/`<>`). The operator is
either attached to its target (`>file.txt`) or space-separated (a bare
operator takes the next word as its target: `> file.txt`). A bare
operator's target must be a plain word: not a separator (`;`, `|`) and not
another operator (`>`, `<<`, `&1`, `%var`) — `cmd >` and `cmd > ;` are
`invalid redirect` parse errors. Targets: a path (globbed like other
words, with the quote mask applied), `%var` (the variable's fd, attached
form: `>%var`), and `/dev/fd/N` / `/proc/self/fd/N` (dup of the open fd
N). `&>file`, `>&1`, `>&%var` (dup the fd the variable refers to, e.g.
`2>&%f`), `2>&-`, here-docs, and here-strings are the other redirect
sources; see below. A second redirect to the same fd on one
command is a `duplicate redirect target` parse error.

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

Forms: `cmd <<DELIM` (attached), `cmd << DELIM` (separate word),
`cmd <<"DELIM"` (quoted), and `cmd <<-DELIM` (the tab-stripping form: leading
TABs are dropped from every body line and from the terminator line — spaces are
kept, and a tab-indented terminator matches; the body is otherwise opaque as
below). An unquoted delimiter runs `$` / `$(…)` / backtick expansion in the
body; a quoted delimiter makes the body literal. `<<""` (empty quoted
delimiter) reads a body up to the next empty line. The delimiter word folds its
unquoted escape pairs (`<<E\OF` delimits `EOF`), and a quoted delimiter keeps
the pair (`<<"E\OF"` delimits the literal `E\OF`).
The body is otherwise opaque: `;`, `&&`, `|`, `#`, `$(…)`, quotes, and
keyword-shaped lines (`fi`, `done`, `}`) are all body content. An empty body
is zero bytes, and no trailing newline is appended — the body keeps the last
line's own newline (`ab\n` is 3 bytes). A here-doc works in pipelines
(`cat <<EOF | wc -l`), in block bodies (if/while/for/case/function), and in
block conditions and cond-list positions (`if cat <<EOF; then …`,
`cat <<EOF && x`), like any other command. When a line carries several
here-docs (`cat <<A && cat <<B`), the bodies are read after the whole logical
line, in operator order, so a body may contain another here-doc's delimiter.

Limitations:

- The REPL buffers incomplete input (here-docs, `if`/`while`/`for`/`case`/
  `function` blocks, a trailing `&&`/`||`, and an unbalanced quote) under a
  `> ` continuation prompt until the construct is complete, then executes it.
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
  a leading `]` as a literal member, and POSIX classes `[[:alpha:]]`,
  `[[:digit:]]`, `[[:alnum:]]`, `[[:upper:]]`, `[[:lower:]]`, `[[:space:]]`,
  `[[:punct:]]`, `[[:xdigit:]]` (also `[[:blank:]]`, `[[:print:]]`,
  `[[:graph:]]`, `[[:cntrl:]]`).
- Matching is per path component: `*` never crosses a `/`. Unquoted `/`
  separates pattern components, which are walked literally (`echo sub/*`).
  A leading `/` makes the pattern absolute; a trailing `/` matches
  directories only and is kept in the results (`echo */` → `sub/`).
- A pattern cannot consume a name's leading `.` unless the pattern starts
  with a literal `.` (`echo *` skips `.hidden`; `echo .h*` matches it).
  `.` and `..` are never glob results, for any pattern (`echo .*` lists the
  real dotfiles only; `echo .?` passes through verbatim).
- Symlinks to directories are followed at intermediate positions
  (`ln -s sub link; echo link/*` works).
- Results are sorted bytewise. A pattern with no matches is passed through
  verbatim (bash default); `shopt -s nullglob` makes it disappear;
  `shopt -s failglob` makes the command fail with `no match: <word>`
  (failglob wins over nullglob).
- `shopt -s dotglob` makes `*`, `?`, and `[...]` match a leading `.`
  (`echo *` then lists `.hidden`); it never lists `.`/`..` on its own.
- `set -f` (`shopt -s noglob`) disables pathname expansion entirely: a
  pattern word is passed through verbatim (`set -f; echo *` prints `*`);
  `set +f` re-enables it.

Quoting: double-quoted bytes always match literally; a fully quoted word is
never expanded. Expansion happens for command arguments (external, builtins,
`builtin`, `become`), `set --` words, user-function arguments, unquoted
`$@` / `$*` fields (which re-glob, like bash), literal `for … in` words,
redirect target words (a `> word` with pattern chars), and the command name
(word 0) — a sole match runs that file; several matches run the first with
the rest as leading arguments. A glob matching a file with the same name as
a function or builtin runs the file (dispatch sees the raw word, not the
expansion). `$(…)` / `$((…))` outputs and `case` words are not expanded.

A redirect target that globs to more than one file is an error:
`echo hi >a*` with two `a*` files fails with `ambiguous redirect`; a target
with no match writes a literal file named after the pattern.

Limitations:

- Backslash follows POSIX #4.1: outside quotes the pair `\X` removes the
  backslash and `X` loses its special meaning, so it is a literal word byte
  protected from word splitting and globbing — `echo a\*` prints `a*`,
  `echo f\oo\.*` globs the literal prefix `foo.` only, `\<newline>` is a line
  continuation, and a trailing `\` at end of input keeps its backslash.
  Accepted divergences (the escape pair is folded at substitution, so the
  word's syntax position is where the pair sits): `echo a\&&b` prints `a&&b`
  (bash backgrounds `a&`), `echo 2\>&1` prints `2>&1` (bash redirects `2>`),
  `echo a\<<X` prints `a<<X` (bash starts a heredoc), `echo \$(echo hi)`
  prints `$(echo hi )` (bash is a syntax error), and `\X=1` runs the command
  `X=1` (fdshell exits 1, bash 127).
- With `nullglob`, an unmatched command name falls back to the literal word
  (`shopt -s nullglob; zzz*` tries to run `zzz*` and fails with `not
  found`); bash drops the command and exits 0.

## Brace expansion

A word containing an unquoted brace group is expanded before tokenization
(bash brace-expansion semantics):

- `{a,b,c}` — a comma group: one word per unquoted top-level comma.
- `{start..end[.incr]}` — a sequence: decimal integers, one-byte values
  (`{a..c}`), and zero-padded integers (`{01..03}`, `{-01..1}`). `incr` may
  be negative (`{1..5..-2}`); `0` means `1` (`{1..10..0}`). Direction follows
  the sign of `end - start`; the step sign is corrected to match.
- Several groups in one word form a cartesian product (`{a,b}{c,d}` →
  `ac ad bc bd`), and a group may nest inside another (`{1..{a,b}3}` →
  `{1..a3} {1..b3}`).

Not expanded (kept verbatim, like bash): quoted braces (`"{a,b}"`,
`a"{b,c}"d`); a group with neither a comma nor a valid sequence (`{a}`);
incomplete sequences (`{..5}`, `{1..}`, `{a..b..c}`); non-decimal terms
(`{0x1..0x3}`, `{1.5..3}`); and `i64` overflow (`{1..100000000000000000000}`).
An unquoted `..` directly before the closing brace does not start a sequence
(`{1..}` stays literal). Empty expanded words drop out on re-tokenization
(`echo {,}` runs `echo` with no arguments; `echo x{,}` → `x x`).

Protected contexts (no expansion, like bash): the assignment word
(`x={a,b}`), `case` words and pattern lists, here-string words, and heredoc
delimiters — both the `<<` operator word and the terminating delimiter line.
Redirect target words *are* expanded: `echo hi >{a,b}` (attached) becomes
two redirects and a `duplicate redirect target` parse error, and
`echo hi > {a,b}` (separated) redirects to the first expanded word with
the rest as arguments — bash reports an ambiguous redirect for both.

Limitations / deviations from bash:

- The expanded words are re-tokenized, so a generated `#` or shell keyword is
  re-recognized: `echo {#a,#b}` starts a comment at `#a` (running `echo` with
  no arguments), and `{if,fi} hi` treats the generated `if` as the `if`
  keyword (a parse error). bash keeps the expanded bytes literal for both
  (`echo {#a,#b}` prints `#a #b`; `{if,fi} hi` runs `if` as a command).
- A single source word that would produce more than 65536 words is a clean
  parse error (`brace expansion produced too many words`) rather than bash's
  allocate-then-fall-back-to-literal.
- Backslash is an ordinary byte outside quotes (see Glob expansion), so it
  never quotes a brace.

## Arithmetic

fdshell evaluates C-style integer expressions in-process (no child). Three
forms share one expression language:

- `$((expr))` — an *expansion*: the word is replaced by the decimal result
  (`echo $((2+3*4))` → `14`).
- `((expr))` — a *keyword command*: evaluates `expr` and sets the exit status
  to `expr == 0` (0 when the value is non-zero, 1 when it is 0), like bash.
  `((1+2)); echo $?` → `0`; `((0)); echo $?` → `1`.
- `let expr [expr …]` — a *builtin*: evaluates each argument as a separate
  expression and sets the exit status from the last one. `let x=3+4; echo $x`
  → `7`; `let i=1 j=2` sets both.

The expression language:

- C operator precedence and associativity: `**`, unary `!`/`~`/`-`,
  `*`/`/`/`%`, `+`/`-`, `<<`/`>>`, comparisons, `&`, `^`, `|`, `&&`, `||`,
  and the ternary `c?t:e`. `&&`/`||` are boolean (result 0 or 1) and
  short-circuit, like bash.
- `++`/`--` increment/decrement on a variable, pre- or post-fix: `x=3;
  echo $((x++)) $x` → `3 4`, `echo $((++x))` → `4`. Postfix binds tightest
  (`x++*2` is `(x++)*2`), prefix binds tighter than `*`. `++`/`--` lex as one
  token only next to a variable name, so `++1` is `+ (+1)` = 1 and `x++--1` is
  `x++ - (-1)` = 2, exactly as bash lexes them.
- The comma operator, lowest precedence: the value is the last operand and the
  earlier ones are evaluated for their side effects. `echo $(( (i=1, j=2, i+j)
  ))` → `3`; `echo $((x=5, x*=2, x))` → `10`. A paren group takes the comma
  level, so `(1,2)+3` → `5`.
- Decimal, hex (`0xff`), leading-`0` octal (`010`), and `base#number`
  literals (`16#ff`, `2#1010`; base 2–64, digits `0–9a–zA–Z`, no
  leading-zero base).
- A variable name (`x`) or `$name` both resolve as the variable's value,
  re-evaluated as an expression (unset/empty is 0); `$$` is the shell pid and
  `$!` the last background pid.
- Assignment `x=3` and compound assignment `x+=3` (and `-=`/`*=`/`/=`/`%=`/
  `&=`/`|=`/`^=`/`<<=`/`>>=`) update the variable and yield the new value. The
  LHS value is read before the RHS is evaluated, so `x+=++x` with `x=3` is
  `3+4` = `7` (bash order).

Limitations / deviations from bash:

- `++`/`--` on a non-variable operand is evaluated with no side effect, so
  `++x++` is accepted (it yields the incremented inner value) where bash
  errors with "assignment requires lvalue".
- `$x++` increments the variable named by `$x`, because fdshell treats `$x` as
  a variable node; bash expands `$x` textually first, so `$(( $x++ ))` is a
  syntax error there.
- A command's arguments are substituted in the forked child, so a `$((…))`
  side effect does not reach the next statement: `x=3; echo $((x++)); echo $x`
  prints `3` then `3` (bash prints `4`). The same command sees it
  (`echo $((x++)) $x` → `3 4`), and the `((…))` keyword and `let` forms run
  in-process, so they persist.
- An empty expression (`(( ))`, `let ""`) is a syntax error, not bash's
  status-1 no-op.
- `((…))` takes no redirections, captures, background form, or pipeline
  position: `((x)) >f`, `((x)) | cat`, and `((x)) &` are clean parse errors.
- A statement that *starts* with `((` is always the arithmetic keyword, so
  `((x)=1` is a parse error rather than an assignment.
- The `$((…))` body is scanned by paren depth and closes at the first `)`
  that brings the depth back to 2, so an inner `(` stays inside the body:
  fdshell accepts `echo $((1,2)+3)` → `5`, `echo $((1,2)*3)` → `6`,
  `echo $((1,2,3)+1)` → `4`, and `echo $((1+2)*3)` → `9` (the `+` form
  predates the comma operator), where bash reports a command-substitution
  syntax error. The `((…))` keyword form stays strict (`((1,2)+3)` →
  "arithmetic command must be `((expr))` with no trailing words") and
  `let "1,2,3)+1"` is a syntax error, like bash.
- No positional parameters, command substitution, or `${…}` inside an
  arithmetic expression.
- `let` evaluates each argument as a separate expression (bash-compatible).

## Shell variables

Plain `NAME=value` statements persist in the shell (`FOO=bar; echo $FOO` →
`bar`); several on one line set them all (`FOO=bar BAZ=qux` persists both).

Leading `NAME=value` words on a *command* are scoped to that one command
(POSIX 2.9.1): the values are expanded against the shell, set for the
command's execution environment, and exported to external children — but
they never persist (`FOO=bar env` shows `FOO=bar`; a following
`echo ${FOO:-unset}` prints `unset`). A scoped `IFS` does not re-split the
command's own words (`IFS=: echo a:b` → `a:b`); the command's environment
does see it. Functions and intercepts (`eval`, `source`, …) run inside the
scoped window, so their bodies see the values, then the shell's previous
values are restored (first-touch restore: `FOO=pre; FOO=1 FOO=2 cd /tmp;
echo $FOO` → `pre`).

Accepted divergences from bash:

- `FOO=bar | cat` is a parse error (`expected command`); bash runs a no-op
  subshell component.
- `FOO=bar if …` runs `if` as a plain (not found) command; bash is a syntax
  error.
- A quoted `"FOO=bar" cmd` is treated as a scoped prefix (fdshell keys on
  the unquoted word, as for bare assignments); bash would exec a command
  literally named `FOO=bar`.
- `eval`/`source` bodies expand against the scoped window (fdshell expands
  intercept arguments at run time, inside the window); bash expands the
  intercept's own words before scoping, so `FOO=bar eval "echo $FOO"` prints
  `bar` here and empty in bash.
- `test`/`[` grouping uses unquoted, quoted, or escaped parens (`[ ( … ) ]`,
  `[ "(" … ")" ]`, `[ \( … \) ]`): the escape pair folds to the literal `(`/`)`
  at substitution, so the POSIX escaped form groups exactly as bash does.

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

A capture target may also be a **bounded array** `var[N]` (tagged: `tagvar[N]`):
received fds then append to the fd array `var` instead of a scalar fd var, up to
`N` entries **in total** — entries captured by earlier runs count toward the cap,
and any fd that no longer fits (or whose tag does not match) is closed
immediately. The form is usable by any command: foreground, background
(`cmd %>%arr[N] &>&x; waitpid &x`), and `wait` arms. Tag matching follows the
scalar form: `%>%arr[N]` accepts an fd of any tag, `%tag>%arr[N]` only fds sent
with tag `tag`. With several captures on one command, each received fd goes to
the **first declared** capture that still has room and whose tag matches — an
untagged capture declared before a tagged one takes the tagged fd:

```shell
# each accepted connection appends to %conns (cap 2); the third
# accepted connection is closed by the shell
builtin accept %l %accept>%conns[2]
builtin accept %l %accept>%conns[2]
builtin accept %l %accept>%conns[2]
for %c in %conns; do handle %c; done
```

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

The POSIX `wait [pid…]` builtin reaps background tasks **by pid** (every task when given
no argument) and sets `$?` to the last reaped exit status — the pid-based complement to the
name-keyed `waitpid &name`. `$!` holds the last background pid:

```shell
builtin false &>&j
wait $!            # reap by pid; $? is the child's exit status
wait               # reap every background task
```

Because `wait` is also the event-case block keyword, the shell disambiguates by what
follows it: a `wait` opens a block when the next word is on a subsequent line, or is a
same-line pattern keyword (`readable` / `writable` / `finished` / `after`); a same-line
pid/`$!`/name, a `;`, a quoted word, or end-of-input is the POSIX builtin (a bare `wait`
at end-of-line in a multi-line script opens a block). In the REPL, `wait` + Enter
therefore runs the builtin (bash-compatible) — start a multi-line block with the pattern
keyword on the line after `wait`.

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

### `times`

`times` prints the shell's and the reaped children's accumulated user/sys CPU times in
bash's layout:

```
	User time	System time
	0.00	0.01
	Children user time	Children system time
	1.23	0.05
```

The children times accumulate as children are reaped (each reap reads the child's `rusage`
and adds it to the running total), so `times` after a `wait` reflects the work those
children did.

### Socket lifecycle (`bind` / `listen` / `accept` / `connect`)

`bind`, `listen`, `accept`, and `connect` give a script the server **and**
client side of an AF_UNIX or AF_INET v4 socket; each result is captured as
an fd variable:

```shell
builtin bind [--type stream|dgram] ADDRESS
builtin listen [--type stream|dgram] [--backlog N] ADDRESS
builtin accept %fd
builtin connect [--type stream|dgram] ADDRESS
```

* `ADDRESS` is the positional: `@name` is the **abstract namespace** (no
  filesystem object — no path TOCTOU, nothing to clean up); `path` is a
  filesystem socket resolved against the CWD — the script owns the path and
  must `unlinkat` it after use (and clear a stale one before re-binding), or
  the stale path is `EADDRINUSE` (98) on the next bind; `--bind ADDR --port N`
  is AF_INET v4. Names and paths are at most 107 bytes.
* Output uses the usual capture forms: `builtin listen @srv %>%l`, or the
  bounded form `builtin accept %l %>%conns[8]` to fill an fd array. At the
  cap, further connections are accepted and closed immediately.
* `accept` blocks until a connection arrives; kernel failures (e.g. `listen`
  on a dgram socket) surface as their errno exit code. Without a capture,
  `accept` accepts and closes immediately (the `pipe` precedent).
* `connect` is the client side: it creates the socket (default `stream`) and
  connects it to the ADDRESS — for `connect` the `--bind ADDR --port N` pair
  names the **peer** endpoint, not a local bind. It returns as soon as the
  peer is queued in the listener's backlog (no `accept` needed on the
  other end — a single script can be both client and server), it never
  creates a filesystem object, and kernel failures surface as their errno
  (e.g. `ECONNREFUSED` 111 for an unbound address). For a remote peer
  `connect` blocks until the handshake completes, so background it
  (`builtin connect … %>%c &>&x; waitpid &x`) when the script has other
  work.

Non-blocking forms are the standard ones: backgrounding
(`builtin accept %l %>%c &>&x; waitpid &x`) or a `wait` arm — the arm child's
`send_fd` appends the accepted connection to the main shell's bounded array:

```shell
builtin listen @srv %>%l
wait
    readable %l %>%conns[1])
        builtin accept %l %>%c
        send_fd %c
        unset %c ;;
    after 1000)
        echo "no connections" ;;
done
```

### Custom AF_UNIX protocols (`sendmsg` / `recvmsg`)

`sendmsg` and `recvmsg` move a **byte payload** together with **any number of fd
variables** in a single AF_UNIX `sendmsg`/`recvmsg` (SCM_RIGHTS) message. They are the
primitive for bespoke socket protocols.

```shell
sendmsg %sock [--msg TEXT | --msgfd %var COUNT] [--fd %var]... [--passcred]
recvmsg [--cred VAR] %sock VAR [%fdvar ...]
```

* `%sock` is a connected socket: an fd variable (`%sock`), or a raw fd number (as in
  `read -u`). Sockets must be **pre-connected** — `sendmsg`/`recvmsg` do not create or
  accept them; `bind`, `listen`, `accept`, and `connect` cover the socket lifecycle
  (above).
* `--msg TEXT` sends the inline payload; `--msgfd %var COUNT` sends the first `COUNT`
  bytes read from an fd variable. Omitting both sends an empty payload (a no-op
  send, useful for readiness signaling).
* `--fd %var` (repeatable) appends that fd to the message; the receiver declares a
  matching `%fdvar` slot per fd, in order.
* `--passcred` enables `SO_PASSCRED` on the sender's socket before the send, so the
  message carries the sender's real credentials (see below).
* `recvmsg` stores the payload in the string variable `VAR`, each received fd in its
  declared slot, and (with `--cred CRED`) the sender's identity in `CRED`.

Sender identity is surfaced as `PID:UID:GID`. Credentials require **both** sides to
opt in: the sender with `--passcred` and the receiver with `--cred CRED` (which
enables `SO_PASSCRED` and harvests the kernel's `SCM_CREDENTIALS`). The kernel
attaches the credentials at send time based on whichever socket had `SO_PASSCRED`
then, so a script can verify the peer's pid (e.g. against the pid it spawned) only
when the sender opted in: a foreign sender that never set `SO_PASSCRED` makes
`recvmsg --cred` silently leave `CRED` unset.

```shell
builtin import_fd 0 %>%peer          # peer's end arrived as our stdin
wait
    readable %peer)
        recvmsg --cred CRED %peer PAYLOAD %data
        echo "peer said $PAYLOAD from ${CRED%%:*}" ;;
    after 2000)
        echo "peer timed out" ;;
done
```

Because `recvmsg` blocks the current shell (like `read`), use a `wait` readable arm — or
backgrounding — to wait for readiness. On EOF (peer closed) `recvmsg` sets `VAR` empty
and exits `1`, so `||`, `if`, and `wait` arms can all observe it.

Payloads are strings: a payload containing a NUL byte is a clean error (binary data
belongs in the fd-transfer path, which these commands already provide).

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
