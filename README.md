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

`--mode` defaults to `mkdir(1)`'s `0777` for `mkdirat` and to `mkfifo(1)` /
POSIX `open(2)`'s `0666` for `mkfifoat` and for `openat2` with `O_CREAT`. The
kernel masks the mode with the process umask, so under `umask 022` a mode-less
`mkdirat` yields a `0755` directory and a mode-less `mkfifoat` / `openat2
O_CREAT` a `0644` file. An explicit `--mode` overrides the default. Each create
builtin reopens the object it just created, so a mode whose masked value lacks
the owner bits that reopen needs (`r|x` for `mkdirat`, `r|w` for `mkfifoat`) is
rejected with an error naming `--mode` and `umask`, and nothing is created.

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

Redirect targets and external commands have no per-command `--dirfd` (bash
syntax), so strict resolves both routes through the shell's **pinned `%CWD`
directory fd** — a real `O_DIRECTORY` fd seeded at startup and re-opened by
`cd` — and bans every filesystem-root path:

- a redirect target (`<`, `>`, `>>`, `<>`, `>|`) that is absolute: the ban
  fires before the glob walk, so an absolute *pattern* is caught too, and a
  relative target is globbed and opened `openat2(%CWD, …)`, so it follows `cd`;
- a program word containing `/` that is absolute; `exec`/`become` inherit the
  ban, and so does command substitution (`$(prog)`);
- a `hash` pin or a `PATH` entry that is absolute: under strict `PATH` must be
  relative (a `:`-separated empty entry means `.`, as in bash), a relative
  entry is joined and opened against `%CWD`, and the program runs from the fd
  of the inode that walk pinned (`execveat`), so a root-escaping `PATH` fails
  loudly instead of half-working.

`cd ..` is the user's own escape hatch: the base follows the shell, exactly as
a relative path does today. Accepted gaps (documented, not banned): `..`
traversal across the base, symlink escapes (a `RESOLVE_*` policy is follow-up
work), `~`/`$HOME` expansion, and word-expansion globbing, which stays
CWD-rooted (bash semantics — the bans above guard its root escapes).

## Redirection

Operators: `>` (write), `>>` (append), `<` (read), `<>` (read and write),
`>|` (clobber: a write that bypasses `noclobber`), `&>` (write fd 1 **and**
fd 2 to one target) and `&>>` (append both). The first five take an optional
numeric fd prefix (`2> file`, `1< file`, `3<> file`, `2>|file`; no prefix
means fd 1 for `>`/`>>`/`>|` and fd 0 for `<`/`<>`). The operator is either
attached to its target (`>file.txt`, `>|file.txt`, `&>file.txt`) or
space-separated (a bare operator takes the next word as its target:
`> file.txt`, `>| file.txt`, `&> file.txt`). A bare operator's target must be
a plain word: not a separator (`;`, `|`) and not another operator (`>`, `<<`,
`&1`, `%var`) — `cmd >`, `cmd > ;` and `cmd &>` are `invalid redirect` parse
errors. Targets: a path (globbed like other words, with the quote mask
applied), `%var` (the variable's fd, attached form: `>%var`, `&>%var`), and
`/dev/fd/N` / `/proc/self/fd/N` (dup of the open fd N). `>&1`, `>&%var` (dup
the fd the variable refers to, e.g. `2>&%f`), `2>&-`, here-docs, and
here-strings are the other redirect sources; see below. A command may carry
several redirections to the same fd;
all of them are applied in the order written, so the **last one to that fd
wins** and it is not an error: `echo hi >a >b` writes `hi` to `b` and creates
`a` empty (the first redirect opened and truncated its target), and
`cat <f1 <f2` reads `f2`. A failing redirection aborts the command, so the
order matters: `cat <missing <f2` exits 1 without reading `f2`.

`set -o noclobber` (`shopt -s noclobber`) blocks a `>` write over an existing
file (`echo hi >f` fails with `cannot overwrite existing file`, and the command
aborts). `>|` is the bypass: `echo hi >|f` truncates an existing `f`, in the
attached, bare, fd-prefix (`2>|f`), `%var` (`>|%f`) and `/dev/fd/N` forms, and
on `exec >|f` (permanent). It opens the file with the same flags as `>` and
never changes the `noclobber` option, so the next `>` is still blocked.
`>&|f` is not a clobber form (bash rejects it; fdshell leaves `>&` as a
non-redirect word and the `|` a pipeline pipe), and `>>|f`, `<|f`, `>||f`,
`> | f`, `&>|f`, a `>|` with no operand and a bare `&>` / `&>>` with no
operand are `invalid redirect` parse errors.

A redirection belongs to the simple command it is written on, and a simple
command includes a **user-function call** and an **in-process builtin** (`cd`,
`eval`, `source`, `local`, `shift`, `times`, `timeout`, `:`, …). For those the
shell applies the redirections to its own fds before the command runs and puts
the fds back when it finishes, in reverse order, so the shell's fds survive the
command: `f() { echo in-f; }; f >a; echo after >b` writes `in-f` to `a` and
`after` to `b`, and `cd /tmp >a` creates `a` in the pre-`cd` directory while the
`cd` itself persists. A `N>&-` target is closed for the command and reopened at
the restore (`f >a 3>&-` runs with fd 3 closed, and fd 3 is usable again after
the call). `exec >file` has no command to run, so its redirection stays: the
shell's fd 1 is the file for the rest of the script. Inside a function body the
restore applies at the call end, so `f() { exec >a; }; f; echo after >b` leaves
`a` empty and `b` holding `after`.

Clobber deviations from bash:

- fdshell never breaks a word at an operator byte, so `echo x a>|b` passes the
  single argument `a>|b` and creates no file (bash breaks the word at `>` and
  clobbers `b` with `x a`). Same class as the existing `a>b` / `echo 2>&1`
  word-break deviations.
- `>|%x` and `&>%x` are fd-variable redirects (fdshell's `%` convention); bash
  writes to a file named `%x`. `>| %x` and `&> %x` (spaced) are
  `invalid redirect` operand errors, as `> %x` is today.
- Parse errors exit 1 with `parse error:` on stderr, where bash exits 2 with a
  syntax error (repo-wide convention).
- `cmd &> | f` is an `invalid redirect` parse error: the bare `&>` refuses the
  `|` as its operand and no file is created (bash exits 2 with a syntax error,
  fdshell exits 1). `cmd &>|f` (one token) is an `invalid redirect` parse error
  too. `cmd &> file` writes `file` with fd 1 **and** fd 2, as bash does.

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

An unquoted delimiter runs `$` / `$(…)` / backtick expansion in the body; a
quoted delimiter makes the body literal:

```shell
cat <<EOF
`echo hi` $(echo yo)
EOF
# → hi yo                       (unquoted: `…` and $( … ) expand)

cat <<'EOF'
`echo hi` $(echo yo)
EOF
# → `echo hi` $(echo yo)        (quoted: the body is literal)
```

Forms: `cmd <<DELIM` (attached), `cmd << DELIM` (separate word),
`cmd <<"DELIM"` (quoted), and `cmd <<-DELIM` (the tab-stripping form: leading
TABs are dropped from every body line and from the terminator line — spaces are
kept, and a tab-indented terminator matches; the body is otherwise opaque as
below). `<<""` (empty quoted
delimiter) reads a body up to the next empty line. The delimiter word is a
word, so it gets quote removal: every `"`/`'` byte is dropped in the same pass
that folds its unquoted escape pairs (`<<E\OF` delimits `EOF`, `<<a'b'c`
delimits `abc`, `<<""Q"x"` delimits `Qx`), a quote written as an escape pair is
data (`<<a\"b` delimits the literal `a"b`), a quoted delimiter keeps the pair
(`<<"E\OF"` delimits the literal `E\OF`), and a quote byte **anywhere** in the
word quotes the delimiter, so the body is literal. A quoted span is one
delimiter word, spaces and all (`<<"a b"` and `<<'a b'` delimit `a b`).
The body is otherwise opaque: `;`, `&&`, `|`, `#`, `$(…)`, quotes, and
keyword-shaped lines (`fi`, `done`, `}`) are all body content. An empty body
is zero bytes, and no trailing newline is appended — the body keeps the last
line's own newline (`ab\n` is 3 bytes). A here-doc works in pipelines
(`cat <<EOF | wc -l`), in block bodies (if/while/for/case/function), and in
block conditions and cond-list positions (`if cat <<EOF; then …`,
`cat <<EOF && x`), like any other command. When a line carries several
here-docs (`cat <<A && cat <<B`), the bodies are read after the whole logical
line, in operator order, so a body may contain another here-doc's delimiter.
The regions are allocated **per physical line, in operator order**: every
consumer of a line's body stream first subtracts the operators that already
claimed that line. A block's text carries only the regions its own operators
claim — the regions written before the block on the same line belong to those
statements, and a heredoc statement after the block on the block's line is
outside the block's span: `cat <<X; if cat <<A; then echo y; fi; cat <<B` +
bodies prints `bodyX bodyA y bodyB` (as bash), for `if`/`elif`/`while`/`until`
and any number of pre-block operators. The allocation applies **per arm**
inside the block too (task #238): in a single-line block the bodies sit past
the closing keyword, so each arm's text is rebuilt from the regions its own
operators claim — `if true; then cat <<T; fi` + body + `T` prints `bodyT`
(rc 0, as bash), and so do the `while`/`until` body (`while true; do cat <<T;
break; done`), the `elif` arm and the `else` arm, an arm with several
here-docs (`then cat <<A; cat <<B; fi`, bodies in operator order) and an arm
below an earlier arm's here-doc (`elif cat <<C; then cat <<T; fi`); in a
multi-line block the arm's own lines already carry the body inline. A here-doc on a
**user-function call** delivers the body to the call, because a function call is a simple command:
`f() { cat; }; f <<EOF` prints the body, and the shell's stdin is back when the
call finishes, so `echo after` after it prints on the terminal (as bash).

Limitations:

- The REPL buffers incomplete input (here-docs, `if`/`while`/`for`/`case`/
  `function` blocks, a trailing `&&`/`||`, and an unbalanced quote) under a
  `> ` continuation prompt until the construct is complete, then executes it.
- Here-docs inside `$( )` / backticks, an `N<<EOF` fd prefix (like `N<<<`
  today), and a `# comment` after the operator on the same line are not
  supported. A `<<` written inside a double-quoted region opens no heredoc at
  all (`echo "$(printf "<<EOF")"` prints `<<EOF`), and a multi-line `$( … )`
  body that carries its own heredoc (`echo "$(cat <<EOF` + body + `EOF` + `)"`)
  does work: the body is the child's own text.
- A body line that is exactly `elif` / `else` (in an if body) or `;;` (in a
  case / wait body) is misread as block structure.
- A keyword block whose text carries a `<<` operator keeps the pre-#137 split, so
  a cond-list operator in such a block is still broken:
  `while cat <<X; do a && b; X` + `done` is a parse error rc 1 (bash rc 2), where
  the heredoc-free `while true; do a && b; done` runs (task #137). The reason is
  that a block's condition bodies travel *inside* its text, and the `&&` written
  in a body line must stay a split point
   (`tests/heredoc.rs::block_heredoc_cond_run_and_and_in_body_splits_part`).
   Task #234 owns the fix.
- The same split hits the **body** of a single-line block (task #238's twin of
  the bullet above): `if true; then cat <<T; fi` + `body && x` + `T` is a parse
  error rc 1, where bash prints `body && x` (rc 0) — the body regions appended
  to the arm's text are re-split on their `&&` before the arm parses
  (`tests/heredoc.rs::single_line_block_body_heredoc_with_and_in_body_still_splits`).
  Task #234 owns the fix.
- A line whose body stream is **incomplete** yields no regions at all, because
  the regions are allocated for all of the line's operators in order: in
  `if true; then cat <<T; elif cat <<E; then echo b; fi` + `bodyT` + `T` the
  `elif` condition's `<<E` has no delimiter line, so the `then` arm is left with
  no body and reports its own missing delimiter (rc 1), where bash prints
  `bodyT` (rc 0) with only a warning about `E`
  (`tests/heredoc.rs::single_line_block_body_heredoc_of_an_incomplete_line_stream_is_empty`).
- A **block trailing redirect** is unchanged by the arm-body delivery:
  `if true; then cat <<T; fi > out` + body + `T` prints `bodyT` and never opens
  `out`, exactly as the heredoc-free `if true; then echo hi; fi > out` does
  (`tests/heredoc.rs::single_line_block_body_heredoc_with_block_trailing_redirect`);
  bash writes the body into `out` and prints nothing.
- A **multi-line** block whose condition carries a here-doc keeps its text verbatim:
  `if cat <<A; then` / `echo y` / `fi` + body + `A` runs `echo y`, `fi` and the body lines as
  commands and the condition prints its region, where bash reports a syntax error (rc 2). Only
  the single-line form (`if cat <<A; then echo y; fi` + body + `A`) is bash-compatible.
- A `<<` is an operator when **any** token precedes it, so a line that opens with a
  redirect has no command word: `> <<EOF`, `>| <<EOF` and `; <<EOF` + body consume
  the body in the scanner and report the operator word — rc 1 `">" not found`,
  `">|" not found`, `"<<EOF" not found` (bash: a syntax error, rc 2). The body
  lines are never executed; only a pipeline `|` right before the `<<` keeps it a
  command word (`true | <<EOF` runs its body lines as commands).

## Group command

`{ list; }` (POSIX #1.2) is the group command: the body runs in the current
shell, with no fork, so its assignment, `cd`, `export` and `exec` survive the
group (`{ x=1; echo $x; }; echo $x` → `1 1`). The body is the statements up to
bash's required `;` before `}`, and the trailing redirections apply to the group
and are undone at its end, so the shell's own fds come back
(`{ echo a; } > out; echo b` puts `a` in `out` and prints `b` on stdout). The
group's status is the status of its last body command, `return` in the body
returns from the calling function, `break` breaks the enclosing loop, `set -e`
exits inside the body, `set -x` traces the body commands (not the braces), and
`eval "{ echo a; }"` runs the group. `{` and `}` are the block-depth `+1`/`-1`,
so a group inside an `if`/`for`/`case`/function body closes at the block's own
keyword, and `echo "$( { echo a; } )"` captures the group output.

A group is a cond-list part, so `{ echo a; } && { echo b; }` prints `a b` and
`{ false; } && echo b` prints nothing (rc 1). A brace glued to a word
(`{echo`, `{a,b}`, `{1..5}`, `{}`) is a command name or brace expansion, and a
brace in an argument position (`echo {` prints `{`) or after a pipeline `|` is a
command word, as in bash. `exec >file` inside a group is permanent: the group is
the current shell, so the redirection is not undone at the group end, as in bash.

A keyword block (`if`/`for`/`while`/`until`/`case`/`wait`) is one cond-list part
too (task #137): an `&&` / `||` written inside the block's text belongs to the
block's *own* cond list, never to the list around it. So
`while [ 1 ] && [ 1 ]; do echo hi; break; done` runs its condition as a cond
list (`hi`, rc 0) exactly like a top-level `a && b` line,
`if [ 1 ] && [ 0 ]; then echo t; else echo f; fi` takes the branch the whole
condition decides, and `while true; do false && echo a; break; done` keeps the
operator in its body. The outer split lands on the operator written *outside*
the block (`while false; do :; done; echo x`), a `{ …; }` group inside a block
condition keeps its per-part rule (`while { true; } && { true; }; do …; done`),
and a `time [-p]` prefix is transparent (`time while [ 1 ] && [ 1 ]; do …; done`).
Only a *command-position* keyword opens a block: written as an argument it is
word data, exactly as a brace is above — `echo if && echo b` prints `if` and `b`,
and `echo x > if && echo b` writes `x` into the file `if` and then runs `echo b`.
Pinned by `tests/cond_blocks.rs` and `tests/loops.rs`. Two shapes stay broken:
an operator *adjacent* to a block (`true && while …; do …; done`, task #232) and
a block whose text also carries a here-doc (task #234, see "Heredocs").

Accepted divergences from bash:

- A group as a pipeline stage (`{ echo a; } | cat`, `echo a| { echo b; }`) and a
  background group (`{ echo a; } &`) are parse errors (`unexpected pipe` /
  `unexpected character`, rc 1, where bash forks the stage and exits 0); in
  `echo a| { echo b; }` the brace after `|` is a command word, so the stage fails
  as `"{" not found` rc 1.
- A brace glued to a word is not a group: `{echo hi; }` runs the command `{echo`
  and reports `"{echo" not found` then `"}" not found` (rc 1); bash's syntax error
  exits 2. `master` agrees.
- `fdshell` has no `bad substitution` diagnostic: in `echo ${{a} && echo b` the
  `${{a}` expands to an empty word, so fdshell prints an empty line then `b`
  (rc 0) where bash reports `bad substitution` (rc 1); `master` agrees.
- A here-doc or here-string on a group (`{ echo a; } <<EOF`, `{ echo a; } <<<x`)
  is a parse error; bash feeds the body to the group.
- A here-doc as the group's **last** body command, followed by `; }` on its own
  line (`{ cat <<EOF` + `hi` + `EOF` + `; }`, run from a script file), prints `hi`
  rc 0: fdshell keeps the `;` as the body's separator so the group runs, where
  bash 5.3.9 reports `syntax error near unexpected token ';'` rc 2. Pinned by
  `tests/group.rs::here_doc_inside_the_group_body`.
- A comment glued to the closing brace (`{ echo a; }# note`) ends the statement:
  fdshell prints `a` rc 0, where bash reports rc 2. Pinned by
  `segment/group/tests.rs::glued_comment_after_the_closing_brace_ends_the_statement`.
- A word after the closing brace (`{ echo a; } echo b`) is a parse error; bash's
  syntax error names the token.
- The unclosed group (`{ echo a`), the empty group (`{ }`) and the missing `;`
  before `}` (`{ echo a }`) are parse errors with fdshell's own messages, and
  `{; echo a; }` fails as `group is missing a closing brace` because the scanner
  splits `{;` into the word `{` and `;` (bash: rc 2 syntax error).
- `set -v` prints the group line as it is read (fdshell prints the input line of
  every statement at execution time); bash prints it when it parses the group.
- Nesting is capped at 100 groups in one construct (`nesting too deep`, rc 1),
  as for the other nested constructs.
- A literal `}` argument inside a block body consumes the block's closing keyword
  (`if true; then echo }; fi` → rc 1, `"fi" not found`), where bash prints `}`; a
  literal `{` argument prints `{` as bash does. The cost of counting every
  `{`/`}` word as the block depth, which is what makes a nested group inside a
  block body close at the block's own keyword.
- `wait` at the end of a line absorbs the group on the next line as its
  event-case body (`wait` + newline + `{ echo a; }` → rc 1 parse error), where
  bash prints `a`.

## Loop control (`break` / `continue`)

`break` and `continue` take an optional positive loop count: `break n` exits `n`
enclosing loops, `continue n` advances `n` enclosing loops to their next
iteration (`break` alone is `break 1`). The count is clamped to the loop nesting
depth at the call site, so `break 5` in one loop exits that loop silently, as
bash (`for a in 1 2 3; do j=$((j+1)); break 5; done` → one iteration). The loop
levels are `for`, `while` and `until`; `if`, `case`, a `{ …; }` group, a
cond-list/pipeline part and a `wait` block in the **loop body** are transparent,
as in bash (`case_clause_is_not_a_loop_level`,
`group_and_if_do_not_count_as_loop_levels`). A `break`/`continue` raised inside
a loop **condition** is that loop's own signal — the condition runs inside the
loop's level, so it is consumed exactly like the body's:
`while if true; then break; fi; do echo body; done; echo end` prints `end`,
`while break; do echo body; done; echo end` prints `end`,
`i=0; while [ $i -lt 3 ] && break; do i=$((i+1)); done; echo i=$i` prints `i=0`,
and `break 2` in a nested loop's condition exits both loops. `while`/`until`/`for`
are the levels; an `if`/`case`/group in a condition stays transparent, so a
control raised there reaches the loop whole (the `elif` arm and a `return` in a
condition propagate the same way). A `continue` in a condition advances the loop
to its next condition test, which is an infinite loop in bash too
(`while continue; do …; done`): deliberately unpinned, and every shape that can
regress to one runs under a bounded child in `tests/loops.rs`. `eval`, `source`
and a group share the caller's depth, so `eval "break 2"` in a nested loop exits
both loops, and a `break` in a forked child (`$( … )`, a pipeline stage) is
consumed by that child's own loops. A leading `+` and leading zeros are accepted
(`break +2`, `break 0100`), as bash's `atoi` is.

Accepted divergences from bash:

- A count of 0, a negative count, a non-numeric count (`break abc`, `break 1.5`)
  or a value over `u32` is rejected: `break/continue count must be a positive
  integer`, rc 1, the script aborts. bash warns `loop count out of range` and
  breaks one level for 0 and negatives (rc 1 from the warning, the shell keeps
  running), and aborts with `numeric argument required` rc 2 for the non-numeric
  case. Pinned by `tests/loops.rs`.
- The count is not expanded: `break $n` is rejected rc 1 here, where bash
  expands `$n` (consistent with `return $n`, which also parses the raw token).
- More than one argument (`break 2 3`) aborts rc 1, matching bash's rc 1 (the
  message differs: bash says `too many arguments`).
- `break`/`continue` outside any loop is an rc-1 abort here (`'break' is not
  inside a loop`, `'continue' is not inside a loop`), where bash warns `only
  meaningful in a … loop` and continues with rc 0. This is fdshell's existing
  accepted outside-loop behavior, and the depth-0 bail fires uniformly: it also
  aborts for a `break`/`continue` nested inside an `if` condition —
  `if break; then echo y; fi; echo after` is rc 1 here, where master printed
  `y` `after` with rc 0 (the control was dropped) and bash warns with rc 0.
  Pinned by `tests/loops.rs::break_in_if_condition_at_depth_zero_aborts_rc1` and
  `…::continue_in_if_condition_at_depth_zero_aborts_rc1`.
- A function body is not a loop boundary here: `break N` inside a function
  crosses it and exits the caller's loops (`f() { break 2; }; for a in 1 2; do
  for b in 1 2; do f; done; done` → both loops exit), where bash reports `only
  meaningful in a … loop` for each call and keeps running. Task #203 owns this
  change; `tests/loops.rs::break_2_in_function_body_crosses_the_boundary_current_fdshell_behavior`
  pins the current behavior as its drift alarm.
- A loop that exits through a `break` in its condition **after its body ran**
  reports the status the `break` inherited, where bash reports 0:
  `i=0; while if [ $i -lt 2 ]; then :; else break; fi; do i=$((i+1)); echo body;
  done; echo rc=$?` prints the same `body` `body` `i=2` in both shells and
  `rc=1` here, `rc=0` in bash (measured 2026-10-13). The loop control itself is
  bash-identical (task #205); only `$?` differs, because `break`/`continue` do
  not zero `$?` here — task #262 owns that, on top of task #164's rule for a
  loop's exit status. `tests/loops.rs::break_in_the_condition_after_the_body_ran_ends_the_loop`
  pins the stdout only, for exactly that reason. A loop whose first condition
  test breaks reports 0 in both shells.

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

Quoting: quoted bytes (in `"…"` or `'…'`) always match literally; a fully
quoted word is never expanded. Expansion happens for command arguments (external, builtins,
`builtin`, `become`, `exec`), `set --` words, user-function arguments,
unquoted `$@` / `$*` fields (which re-glob, like bash), `for … in` list words
(after their substitution — see "For-list words"), redirect target words (a
`> word` with pattern chars), and the command name (word 0) — a sole match
runs that file; several matches run the first with the rest as leading
arguments. A glob matching a file with the same name as a function or builtin
runs the file (dispatch sees the raw word, not the expansion). `case` words
are patterns, never pathnames.

A redirect target that globs to more than one file is an error:
`echo hi >a*` with two `a*` files fails with `ambiguous redirect`; a target
with no match writes a literal file named after the pattern.

Limitations:

- Backslash follows POSIX #4.1: outside quotes the pair `\X` removes the
  backslash and `X` loses its special meaning, so it is a literal word byte
  protected from word splitting and globbing — `echo a\*` prints `a*`,
  `echo f\oo\.*` globs the literal prefix `foo.` only, `\<newline>` is a line
  continuation, and a trailing `\` at end of input keeps its backslash.
  The pair is folded at every word surface, not only at substitution, so a
  word that never goes through substitution looks up its folded text:
  `e\cho hi` runs `echo`, `echo hi > a\*b` writes the file `a*b`,
  `for x in a\ b` binds the one word `a b`, `exec e\cho hi` replaces the shell
  with `echo`, `timeout 1 e\cho hi` runs `echo`, `hash a\*` prints the path of
  the `PATH` file named `a*`, and `\X=1` runs the `PATH` file named `X=1` (when
  it is missing fdshell exits 1, bash 127).
  Accepted divergences (the pair stays in the token text, so the word's syntax
  position is where the pair sits): `echo a\&&b` prints `a&&b` (bash
  backgrounds `a&`), `echo 2\>&1` prints `2>&1` (bash redirects `2>`),
  `echo a\<<X` prints `a<<X` (bash starts a heredoc), `echo \$(echo hi)`
  prints `$(echo hi )` (bash is a syntax error), `alias a\*=echo` stores the
  alias name `a\*` so `a\* hi` runs it (bash stores `a*`, the lookup misses and
  it reports `a*: command not found`), the `builtin`/`command` keyword and the
  `if` keyword are recognized on the raw token so `b\uiltin echo hi` looks up
  the command `builtin` and fails (bash prints `hi`), and an identifier
  carrying a pair is accepted here (`for x\y in a; do …` and `export x\*` run;
  bash rejects them as not valid identifiers — task #166).
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
Redirect target words *are* expanded: `echo hi >{a,b}` (attached) becomes two
redirects to fd 1 applied in order, so the last one wins (`b` gets `hi`, `a` is
created empty); `echo hi > {a,b}` (separated) redirects to the first expanded
word with the rest as arguments. bash brace-expands neither form: a redirect
target is not brace-expanded there, so both report `ambiguous redirect` (rc 1).

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

`$((expr))` is a word expansion, on the surfaces `$(…)` expands on: the command
word (word 0, so `$((1+1))` looks up the command `2` — see "The command word"),
command arguments (external, builtins, `builtin`, `become`, `exec`, `timeout`),
the bare and scoped assignment value (`x=$((1+2))`, `FOO=$((1+2)) env`),
`local`'s value, `set --` words (`set -- $((1+1)) c; echo "$*"` → `2 c`), a function
call's arguments, `case` words, here-string words, and `test`/`[` operands. It
expands mid-word and inside double quotes (`echo x$((1+2))` → `x3`,
`echo "a $((1+1)) b"` → `a 2 b`) and the body is whitespace-tolerant
(`echo $(( 1 + 2 ))` → `3`). No child is forked for the arithmetic itself; the
`++` side effects of a word expanded in a forked child follow the third bullet
of the deviations list.

The body is walked once **before** it is lexed, and each `$(…)` and nested
`$((…))` in it is spliced in place, so the parser is handed one expression
(bash's expand-then-evaluate model): `echo $(( $(echo 1) + 1 ))` → `2`,
`echo $(( $(echo 1+1) ))` → `2` (the capture is re-parsed as arithmetic, not
split into words), `echo $(( $((1+2)) * 3 ))` → `9`. The scan is quote- and
paren-aware — a `(`/`)` inside double quotes in the inner command does not move
it (`echo $(( $(true "a)" && echo 5) + 1 ))` → `6`) — and `%` keeps its
arithmetic meaning (`echo $(( 5 % $(echo 3) ))` → `2`), which is why this is a
pass of its own and not the generic word substitution. The `((…))` keyword form
goes through the same pass (`(( $(echo 1) + 1 )); echo $?` → `0`). A capture
obeys `set --stdout-capture-limit` and over the limit fails the command with
`command substitution inside an arithmetic expression failed`; nesting shares
the shell-wide nesting cap; a body the scan cannot close is
`unclosed subexpression parenthesis`. `let` is not part of this pass: its
arguments are substituted at word level like any command's
(`let x=$(echo 1)+1` sets `x` to `2`).

Nothing else is expanded in the body. The arithmetic language has no quoting,
no comments and no command form of its own, so a `$(…)` inside double quotes,
a backtick body, a `$1` and a `${…}` are all syntax errors. A variable's value
is re-evaluated as an expression and never word-expanded: `x=1+1; echo $((x))`
→ `2`, a value that is not an integer expression is
`variable x is not an integer` (rc 1), and `$(…)` written in a value is
never run as a command.

In a `for … in` list a `$((…))` is expanded wherever it sits in the word,
exactly as in a command argument (the list runs the same substitution
pipeline — see "For-list words"): `for i in $((1+2)) $((3*3))` iterates over
`3` then `9`; `i=1; for x in $((i++)); do :; done; echo $i` → `2`;
`for i in 1$((1+2))` → `13` and `for i in $((1+1))$((2+2))` → `24`. A
whole-word `$((…))` is always exactly one iteration, because a rendered
number contains no `IFS` byte and no pattern char, so it neither splits nor
globs.

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
- An empty expression (`(( ))`, `let ""`, `echo $(( ))`) is a syntax error, not
  bash's status-1 no-op (`echo $(( ))` prints `0` there); the same holds for a
  body that expands to nothing, `echo $(( $(true) ))` (bash `0`).
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
- The body has no quoting: a `$(…)` inside double quotes
  (`echo $(( "$(echo 1)" + 1 ))`) and a backtick command body are arithmetic
  syntax errors, where bash expands both to `2`. A quote is a tokenizer
  concept and the body is never tokenized, so quotes quote nothing here:
  `echo $(( '1' + 1 ))` is an arithmetic syntax error, and the span's bytes
  (including both `'`) are passed to the arithmetic parser like everywhere
  else the body text is passed verbatim.
- No positional parameters and no `${…}` inside an arithmetic expression: with
  `set -- 5`, `echo $(( $1 ))` and `echo $(( ${1} ))` are syntax errors, where
  bash expands both to `5`.
- A redirect target word is not arithmetic-expanded: `echo hi >f$((1))` writes
  the file `f$((1))` (bash writes `f1`) — task #213, tracked by task #278. The
  command name (word 0) is expanded, so `$((1+1))` looks up the command `2` and
  reports `"2" not found` at rc 1, where bash runs the command `2` and reports
  127 (see "The command word").
- `export` does not expand its assignment value: `export E=$((1+2))` exports
  the text `$((1+2))` (bash exports `3`) — task #214.
- Nested `$((…))` shares the shell-wide nesting cap: in a command argument
  101 nested forms evaluate and the 102nd reports
  `arithmetic expression nested too deeply; reduce the nesting of $((...))
  expansions` at rc 1, where bash has no such cap.
- `let` evaluates each argument as a separate expression (bash-compatible).

## Shell variables

Plain `NAME=value` statements persist in the shell (`FOO=bar; echo $FOO` →
`bar`); several on one line set them all (`FOO=bar BAZ=qux` persists both).
A value word is expanded **with its own quote mask** — the word's per-byte mask
sliced at the `=` — so a `\<char>` pair written inside quotes keeps its
backslash (`x="a\ b"` stores `a\ b`, as bash) and a quoted pattern byte stays
literal (`x="a[b"` with `${x#"["a]}` strips nothing); the expansion is a single
word, with no field splitting and no pathname expansion (`x="$(printf "a b")`
stores `a b`, `x="a *"` stores `a *`).

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

`local NAME[=value] …` scopes string variables to the current function call.
The value is expanded with no field splitting and no pathname expansion (the
bare-assignment rule, so `local x=*` stores the literal `*`); a bare `NAME`
declares the variable unset for the call; and every shadowed value — `IFS` and
the exported copy included — is restored when the call returns, first-touch
(`v=pre; f(){ local v=1; local v=2; echo $v; }; f; echo $v` → `2` then `pre`).
A local is visible to children forked inside the call
(`export E=env; f(){ local E=loc; env | grep ^E=; }; f` → `E=loc`), and the
scoping is dynamic: a callee sees its caller-call's local unless it shadows it
(`v=1; g(){ echo $v; }; f(){ local v=2; g; }; f` → `2`). `local` with no
arguments lists the call's locals, and `local` outside a function is an error
(rc 1, `local: can only be used in a function`).

An unset parameter expands to the empty string (POSIX 2.6.2), in every form:
`echo "[$undefined]"` → `[]`, `echo ${unset}x` → `x`, and `${#undefined}` → `0`
(the length of an unset parameter is 0). `set -u` (`nounset`) makes an unbound
parameter an error instead (rc 1, `x: unbound variable`), for `$name`, `${name}`,
`${#name}` and `${!name}`; `set +u` restores the empty rule. An indirect
`${!name}` whose name is itself unset is an error (rc 1,
`undefined: invalid indirect expansion`), as bash.

`set` short flags **cluster** in one word: every byte after the sign is applied
with the word's sign, so `set -eu` sets errexit *and* nounset and `set +ef`
clears noglob *and* verbosity; a repeated byte (`set -ee`) is harmless. The
accepted flag bytes are `e u f v x`; a signed word containing any other byte is
an error naming the whole word (`set -ez` → rc 1,
`set: unknown option '-ez'`) and **no** flag is applied — the word is validated
before the first one is set.

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
- `local` with no arguments prints `local NAME=value` lines (the POSIX/dash
  form); bash prints `declare -- NAME="value"`. A declared-unset local prints
  `local NAME`.
- `local` scopes string variables and their exported copies only. fd vars, fd
  arrays and tasks are untouched (fd scoping is task #46), so `local %x` is
  rejected as an invalid name rather than silently doing nothing.
- bash validates identifiers (`local a*b=1` → "not a valid identifier");
  fdshell keeps its assignment-name leniency (any non-empty name without the
  `%` prefix), so `local a*b=1` is accepted.
- `local -x` (a bash option word) is rejected as a name: fdshell rc 1, bash
  enables `-x` for the call and returns rc 0.
- a `$(…)` fork inherits the function frame, so `local` works inside a command
  substitution in a function; bash refuses it in a subshell.
- a user function named `local` shadows the builtin (fdshell resolves functions
  before intercepts, as it does for every other command word).
- redirects on `local` are applied for the command (and restored when it
  finishes); captures on it are rejected (`CapturesNotSupported`), as for every
  other intercepted builtin.
- captures (`%>%x`) on the in-process commands stay rejected, while their
  redirections are applied: a capture needs a forked child to send the fd over
  the shell socket, and these commands do not fork.
- a redirection on a **function definition** is not stored and not applied:
  `f() { echo x; } > a` parses `> a` as its own command word (rc 1,
  `">" not found`) where bash applies it to the definition; `2> a` after the
  closing brace behaves the same. The same holds for a scoped assignment
  (`a=1 > a` → the redirect word becomes a command) and for `unset`/`umask`,
  whose redirect word is read as a keyword argument (parse error, rc 1).
- the bash/ksh keyword form `function f { echo hi; }` is accepted, in every definer
  shape (`function f {`, `function f() {`, `function f () {`, `function f ( ) {`),
  and its divergences are the POSIX `f() { … }` form's, verbatim: `function f` with
  no brace is rc 1 `parse error` (bash rc 2), an empty body `function f { }` runs
  rc 0 (bash rc 2), a trailing run after the closing brace
  (`function f { echo hi; } f`) is dropped (bash rc 2), a definer word after the
  name is ignored (`function f x { echo hi; }; f` → `hi` rc 0, bash rc 2),
  `function() { echo fn; }` defines the function `function` (bash rc 2), and the
   name leniency keeps an option-shaped word (`function -a f { echo hi; }` defines
   `-a`, rc 0) and a quoted name (`function "f" { echo hi; }; f` → `hi` rc 0; bash
   rejects both).
- a `)` is the tokenizer's separator token, so a definer word ends at a glued `)`
   and `)` is always a word of its own: `function f)g { echo hi; }; f` defines `f`
   (`hi`, rc 0; bash rc 2), while a `)` glued to the keyword leaves no definer name
   and the text runs as commands (`function)f g { echo hi; } && echo done` → rc 1,
   `"function" not found` + `"}" not found`, nothing dropped; bash rc 2), exactly as
   the spaced twin `function ) f g { echo hi; } && echo done` does.
- `function` is a reserved word, so `alias function=echo` never replaces the keyword
  position (as bash); a `function` word with no definer name is the missing command
  (`"function" not found`, rc 1) and `function f; echo after` is rc 1 `parse error`
  (bash rc 2 syntax error for both). `function f }{ echo hi; }` is pathological:
  the byte layer claims the span, the token layer sees no `{` token, so the text
  runs as commands (rc 1, `"function" not found`; bash rc 2, as `f() }{ … }`).
- within one command `> a 2>&1` copies the fd 1 the shell had **before** the
  command: every redirect source is resolved up front, so `2>&1` cannot see the
  `> a` target. Bash applies as it goes and `2>&1` lands on the file `a`
  (tracked by task #183).
- `shift` with no arguments returns 0 (`shift >a` creates `a` at rc 0); bash
  returns 1 for the shift itself (`rc=1`) and still creates the file.
- `local NAME` (no `=`) leaves the variable unset for the call, and `local`
  outside a function is an error (rc 1), as bash.
- `${}` (an empty name) stays literal (`echo "[${}]"` → `[${}]`); bash rejects
  it (`bad substitution`, rc 1). An unclosed `${name` stays literal too; bash
  is a parse error (rc 2).
- `${!}` (an empty indirect name) stays literal too, at rc 0
  (`echo "[${!}]"` → `[${!}]`, `[${!}x]` → `[${!}x]`); bash expands it to the
  empty string (`[]` and `[x]`).
- `${!name}` of a name bound to the empty string is an error on both
  (`[${!p}]` with `p=""`), but the wording differs: bash says
  `: invalid variable name`, fdshell `: invalid indirect expansion`. Same rc 1.
- An indirect target that is **not a valid variable name** expands to the empty
  string, silently, at rc 0: with `p="a b"` and with `p=a-b`, `[${!p}]` prints
  `[]` where bash rejects the name (`a b: invalid variable name`, rc 1) and
  prints nothing. `p="-"` follows the same rule: fdshell looks for a variable
  literally named `-` and expands to empty, where bash resolves its special
  `$-` parameter (`[hBc]`, rc 0; fdshell's `$-` is its own flag bytes, empty by
  default — task #177). Under `set -u` fdshell bails `!p: unbound variable` at
  rc 1 for every one of the three (bash keeps `NAME: invalid variable name` rc 1
  for the first two, and
  prints `[huBc]` at rc 0 for `-`). Pinned by `tests/unset_expand.rs`:
  `indirect_target_that_is_not_a_name_is_empty`,
  `indirect_target_that_is_the_dash_name_is_empty` (task #172).
- The `$_` arm is not nounset-checked (unobservable in `-c` mode: `set -u`
  binds `_` exactly as bash does, so `set -u; builtin echo "[$_]"` prints
  `[-u]` on both).
- Arithmetic variables are not nounset-checked: `set -u; echo $((unsetv + 1))`
  prints `1` here, where bash exits with `unsetv: unbound variable` (tracked by
  task #170).
- A `%name` fd variable that is unbound stays literal (`%nosuchfd` prints
  `%nosuchfd`). The fd namespace is a fdshell extension, not a POSIX parameter.
- `%name` expands to a fd number only when `name` starts with an ASCII letter,
  digit or `_` **and** is a bound fd variable; a `%` followed by anything else
  (`%`, `-`, punctuation, end of word) is a literal `%` that consumes just the
  trigger byte.
- `%%` is **not** a shell escape (task #155): both bytes reach the command, so
  the `printf` builtin interprets its own `%%` — `printf '%%'` prints `%` and
  `echo '%%'` prints `%%`, as in bash.
- The literal-`%` escape is the POSIX #4.1 pair **outside** quotes: `\%name`
  prints `%name` even with `name` bound (`echo [\%s]` → `[%s]`). Inside a quote
  span there is no escape pair, so a quoted `%name` is substituted too — an
  accepted divergence (task #252): with fd `s` bound, `printf "%s\n" hi` prints
  the fd number where bash prints `hi`, and `printf '%%s' x` becomes the
  format `%<fdnum>` (`invalid format string`, rc 1) where bash prints `%s`.
- The pattern word of `${name#pat}` and friends is **not** expanded:
  `${v#$(printf abc)}` and `${v#$v}` stay literal (the pattern bytes are matched
  as written), and the brace reader stops at the first `}`, so a pattern
  containing `}` is unreachable: with `v=abcabc`, `${v#a\}b}` prints
  `[abcabcb}]` (the `}` closes the brace, so the `b}` tail is literal) where
  bash prints `[abcabc]`. bash expands `$…`/`$(…)`/`$((…))` inside `${…}` first
  (task #174).
- fdshell's quote rule is byte-level, so a quoted pattern byte is literal and a
  fully quoted word masks every pattern byte: `printf "[%s]" "${v#a*c}"` prints
  the whole value, where bash (which removes the enclosing quotes before
  matching) prints `abc`. The unquoted forms match bash.
- A pattern word must quote its spaces, because fdshell splits words at an
  unquoted space: `sp="a b"` with `printf "[%s]" "[${sp#* }]"` prints `[[a b]]`
  where bash prints `[b]`, and the unquoted form `r=${sp#* }` ends the word at
  the space, so the trailing `}` is run as a command
  (`failed to resolve command path: "}"`). A pattern containing a space is
  unreachable.
- Single-character operators are not implemented: bash `${v+x#y}` prints `x#y`
  (the `+` operator with the word `x#y`), while fdshell reads the whole name
  `v+x`, which is unset and expands to empty. Only the colon-prefixed forms
  (`:-` `:=` `:+` `:?`) and the pattern forms (`#` `##` `%` `%%`) are operators.
- A pattern operator at index 0 (`${%x}`, `${%x#y}`, `${%}`) leaves an empty
  name, so the braced content is read as the parameter name: nounset off expands
  it to the empty string at rc 0 where bash rejects every form (`bad
  substitution`, rc 1); under `set -u` fdshell bails rc 1 naming the content
  (`%x: unbound variable`) where bash says `bad substitution` — same rc,
  different wording. `${#v#a}` reads the name `v#a` and prints `0` at rc 0,
  where bash is a `bad substitution` (task #173 owns the `${#name}` arm).
- An indirect target whose **value** is not a valid variable name is just an
  unset parameter for the **colon** family, so the family's word supplies the
  value: `p="a b"` with `printf "[%s]" "${!p:-z}"` prints `[z]` at rc 0, where
  bash rejects the name (`a b: invalid variable name`, rc 1) and prints nothing.
  The plain form is #172's rule. Pinned by
  `indirect_target_that_is_not_a_name_with_a_word`.
- The **empty indirect name** (`${!…}`, the scan name is exactly `!`) is unset
  for the colon family, as in bash (`${!:-z}` → `z`, `${!:+z}` → empty, rc 0
  with and without `set -u`), and keeps fdshell's own wording/rc on the two
  forms bash rejects: `${!?m}` is rc 1 `?m: invalid indirect expansion` (bash
  `${!?m}: bad substitution`) and `r=${!:=z}` assigns at rc 0 (bash rc 1,
  `$!: cannot assign in this way`). Pinned by
  `degenerate_empty_indirect_name_with_colon_operators`.
- A **braced positional** expands like the bare one (task #177): `${1}` is `$1`,
  `${0}` is the program name, leading zeros are allowed (`${01}` is `$1`), and
  every operator family acts on the parameter (`${1#c}` strips the value). An
  out-of-range index is unset, and `set -u` bails it naming the index, so the
  plain and pattern families agree with bash.
- The braced `${@}` expands like `$@`/`$*`: the positionals joined by the first
  `IFS` byte (a space when `IFS` is empty), because a braced arm returns one
  string. bash's per-positional word form is the whole-word `"$@"` arm
  (task #219): `IFS=; set -- "a b" c; printf "[%s]\n" "${@}"` prints `[a b c]`
  here, where bash prints `[a b]` and `[c]`; `IFS=,` with
  `printf "[%s]" "${@}"` prints `[a,b,c]` here, `[a][b][c]` in bash. The same
  one-string limit is what the pattern operators act on, so they strip the
  **joined** string where bash strips each positional word: `IFS=,; set -- a b c;
  printf "[%s]" "${*#a,}"` prints `[b,c]` here and `[a,b,c]` in bash
  (`${@#a,}` likewise).
- fdshell's `$-`/`${-}` prints fdshell's own flag bytes (`u`, `e`, `f`, `v`, `x`;
  **empty by default**), so it can be empty where bash's is `hBc`/`huBc`. That
  shows in `:=`: `set --; printf "[%s]" "${-:=q}"` is rc 1
  `-: cannot assign in this way` here, where bash prints its flags at rc 0; and
  in `:?`: `${-:?m}` is rc 1 `-: m` where bash prints `[hBc]` at rc 0.
- `:=` rejects a positional or special target (bash's rule) with the same rc,
  naming the parameter without bash's `$` prefix: fdshell `5: cannot assign in
  this way`, bash `$5: cannot assign in this way`. A parameter that holds a
  non-empty value keeps it, so `${1:=q}` prints `$1` and writes nothing.
- A digit run too large for `usize` keeps fdshell's `TooLarge` bail (rc 1) in
  **both** forms; bash expands the braced form to the empty string at rc 0
  (and under `set -u` bails `99999…: unbound variable`). The bare form is
  pinned by `tests/herestring.rs::here_string_expansion_failure_is_reported`.
- An **indirect target** keeps the plain variable lookup, so a target name that
  names a parameter resolves to nothing here: with `set -- a b c; v=1`,
  `${!v}` prints `[]` where bash prints `[a]`, and `${!#}` keeps fdshell's rc 1
  `: invalid indirect expansion` where bash prints `[c]`. This is #172's
  `-`-target rule extended to digits (task #260). The **colon** family reads the
  same unbound target and its word supplies the value, so `p=1` with
  `${!p:-z}` prints `[z]` here where bash prints `[a]`; `:+` and `:?` follow
  the same face.
- The **write** face of the same rule: `:=` resolves `!name` to the name its
  value names and then refuses to assign a parameter, so `p=1` with
  `${!p:=q}` is rc 1 `1: cannot assign in this way` and `p="-"` with
  `${!p:=q}` is rc 1 `-: cannot assign in this way`, where bash prints `[a]`
  and `[hBc]` at rc 0 (task #260 owns resolving the target as a parameter).
  Pinned by `indirect_target_naming_a_parameter_refuses_assign`.
- Bare `set` lists `$1..` (raw values) before the variables; bash (measured
  5.3.9, in `-c`, script and `-i` modes) lists only variables and functions,
  no positional parameters at all.
- `$0` names the invocation (`fdshell`, or `sh` for `-c` with no extra args);
  bash's `-c` mode puts `bash` there, and with extra args bash's `$0` is the
  **first extra argument** (argv semantics). fdshell keeps every extra argument
  as a positional, so `-c` mode never numbers an extra arg as `$0`.
- `set` clusters accept five flag bytes (`e u f v x`); bash accepts many more
  (`-a -b -C -h …`), so `set -C` is an fdshell error where bash enables noclobber.
  Only the first word is a cluster (`set -eu -f` applies `-eu`, ignores `-f`)
  and `-o NAME` cannot ride inside one (`set -eo pipefail` is an fdshell error);
  task #249.
- An unknown flag byte stops the script at the failing `set` (rc 1); bash
  reports status 2 for it and runs the rest of the script.
- fdshell names the whole word (`set -abc` → `set: unknown option '-abc'`);
  bash names the first unknown byte (`set: -c: invalid option`).
- `set -F` lists fd variables (fdshell's fd-list extension); bash rejects `-F`
  as an invalid option.

### Positional parameters

`$1..$N` are the positional parameters, stored 0-based and read 1-based
(`$1` is the first stored word). `$0` is **not** a slot of that store: it is the
program name, seeded by the CLI — the script path in file mode (argv[1]),
`sh` for `-c` with no extra args, `fdshell` for `-c` with extras and for the
REPL. Nothing rebinds it: `set -- words`, a function call, `source f words` and
`shift` all rebind `$1..` and leave `$0` alone. `$0` is always bound, so `set -u`
never reports it, in **both** forms: the bare `$0` and the braced `${0}` are the
program name (task #177).

`$#` counts the positionals and `$@`/`$*` iterate exactly them, so a function
sees its arguments and never its own name
(`f() { echo "$@"; echo $#; }; f a b` → `a b` then `2`). Out-of-range `$N`
expands to the empty string; under `set -u` it is an error naming the index
(rc 1, `4: unbound variable`).

The braced forms read the same parameters through the same lookup, so nothing is
numbered differently inside `${…}`: a digit run is the positional (`${1}` = `$1`,
`${01}` = `$1`, `${0}` = `$0`) and the single-byte specials `?`, `$`, `@`, `*`,
`-` expand exactly as the bare form does (see "Parameter expansion"). `${#}` is
the one exception: in a word the length arm owns `${#…}` and prints the length of
the empty name, `0`, where `$#` is the positional count (task #173/#259). The
pattern and colon operators act on the positional value
(`set -- a b c`: `${1#c}` → `a`, `${3%b}` → `c`, `${2:-z}` → `b`), the nounset
rule is the shared one (an out-of-range index is unbound: `${5}` → rc 1,
`5: unbound variable`), and `:=` refuses to assign to a parameter.

```
set -- a b c; echo "[$1][$2][$3][$#]"     → [a][b][c][3]
set -- a b c; shift; echo "[$0][$1][$#]"  → [sh][b][2]   (`shift` never moves `$0`)
f() { echo "$1|$#|$0"; }; f a b           → a|2|sh       (the name is not a positional)
set -- a b c; echo "[$1$2${3}]"           → [abc]        (the braced `${3}` is `$3`)
set -- a b c; echo "[${1}][${1#c}][${0}]" → [a][a][sh]
set -- a b c; echo "[${?}][${$}][${*}]"   → [0][<pid>][a b c]
```

### Quoting

A quoted byte never splits, never globs, and never expands. Three forms:

- `'…'` (a **single-quote span**): both quote bytes are removed and every byte
  between them is literal. There is **no escape pair inside a span** (POSIX
  2.2.3), so a backslash is an ordinary byte and shields nothing; `$`,
  backtick, `*`, `?`, `[`, `;`, `|`, `&`, `>`, `<`, `(`, `)`, `{`, `}`, `#`
  are all data. Spans glue to neighbouring words into one word, and `''` is
  one empty word.
- `"…"` (a **double-quote span**): both quote bytes are removed; `$`,
  backtick and `\` keep their meaning there, everything else is literal, and
  `\$`/`\\`/`` \` `` collapse to `$`/`\`/`` ` `` while any other `\X` keeps
  its backslash (POSIX 2.2.2).
- `\X` outside quotes: an **escape pair** — the backslash is removed and `X`
  loses its special meaning (POSIX 2.2.1).

Quote state is a byte-level property of the source text, so the two kinds nest
as data, never as toggles: a `'` written in `"…"` is a literal `'`, a `"`
written in `'…'` is a literal `"`, and a `\` written in `'…'` is a literal `\`.

```
echo 'a b'            → a b        (quotes stripped, one word: no IFS split)
echo 'a"b'            → a"b        (a `"` is data in a span)
echo "a'b"            → a'b        (a `'` is data in `")
echo 'a\b'            → a\b        (no escape pair inside a span)
echo '\'              → \          (a span's backslash is that byte)
echo '\\$v'           → \\$v       (no expansion: `$v` is bytes)
echo 'a$b'            → a$b
echo ''               →            (one empty word: `set -- ''` → `$#` = 1)
echo 'a'b'c'          → abc        (spans glue into one word)
echo 'a b'c           → a bc
echo 'a|b'            → a|b        (no pipeline)
echo 'a;b'            → a;b        (no statement split)
echo '$(echo hi)'     → $(echo hi) (no command substitution)
echo '`echo hi`'      → `echo hi`  (no backtick command substitution)
echo "`echo hi`"      → hi         (backtick command substitution runs)
echo "\`"             → `          (an escaped backtick is literal)
echo '{a,b}'          → {a,b}      (no brace expansion)
echo 'ca?t'           → ca?t       (no globbing, with files `ca1`, `ca2`)
echo '%%'             → %%         (no percent escape: `%%` is data)
echo x'y' # comment   → xy         (a `#` in a span is data, the comment is not)
printf '%q\n' 'a b'   → a\ b       (fdshell's own quoting round-trips)
alias q="echo a'b'"; q  → ab
cat <<'EOF'           → the body verbatim: `$v`, `\`, `*`, `` ` `` unexpanded
cat <<'a b'           → the body, terminated by the line `a b` (a span is a word)
cat <<a'b'c           → the body, terminated by `abc` (every quote byte is removed)
cat <<'x'y'z'         → the body, terminated by `xyz` (the word is quoted: no expansion)
'time' ls            → /usr/bin/time ls   (a quoted keyword is a command word)
```

A quoted word is never an alias name (`'ls'` is not the `ls` alias) and never a
block closer (`if true; then echo 'fi'; fi` prints `fi`). A quoted word that
*contains* a keyword is a command word (`echo 'done'` runs `echo` with the
argument `done`).

Accepted divergences:

- `$'…'` is **not** ANSI-C quoting (task #156): fdshell reads it the POSIX-sh
  way, so `echo $'x'` prints `$x` where bash prints `x`. The `$` is kept
  deliberately — dropping it would expand the variable `x` instead.
- A quoted `case` pattern has its quotes stripped but its bytes still glob
  (task #246, pre-existing for `"` as well).
- A quoted *block opener* at the start of a statement (`'if' x`, `"while" x`)
  is still parsed as the keyword (task #247); bash runs it as a command word.
- An unpaired quote (of either kind) is a parse error reported at the quote
  byte, rc 1.
- Quotes written inside a `${ … }` or backtick **body** are markers, not
  content: the region scan drops both quote bytes and the body text is then
  read as expansion text, so a span's protection does not survive there (see
  "Nested regions inside double quotes"). `v=; echo "${v:-'a b'}"` prints `a b`
  where bash keeps the single quotes literal (`'a b'`), and a literal `}`
  written in an alternate word diverges (`echo ${v:-'a\}b'}` → `a\\b}`, bash
  `a\}b`). The common unquoted case `echo ${v:-'a b'}` matches bash exactly.

### Word splitting (`IFS`)

Field splitting applies to the *result of an expansion* (POSIX 2.6.5), never to
the bytes of a word that performed none: `IFS=,; echo a,b` prints `a,b`, and
`IFS=:; set -- a:b` has one positional.

- runs of `IFS` whitespace collapse to one delimiter and are trimmed at both
  ends of the expansion;
- every non-whitespace `IFS` byte delimits a field, empty fields included:
  leading and middle empties are kept, and a delimiter at the very end of the
  expansion produces no trailing empty field;
- a delimiter merged with an `IFS` whitespace run is trimmed with it
  (`IFS=" ,"; set -- $v` with `v="a, "` → one field `a`);
- bytes produced by an expansion inherit the mask bit of its trigger byte, so a
  quoted byte never delimits, and a fully quoted word is one word;
- `IFS=` disables splitting: the expansion is one field, spaces included;
- an unquoted expansion producing no fields produces no words at all
  (`IFS=,; set -- $(true)` → `$#` = 0), while a word containing quotes keeps
  one empty word (see the quoted-empty rule under "For-list words").

```
IFS=,;  builtin printf "[%s] " a,b          → [a,b]        (literal: no split)
IFS=,;  builtin printf "[%s] " x"a,b"       → [xa,b]       (quoted bytes never delimit)
IFS=,;  builtin printf "[%s] " $(printf "a,,b")  → [a] [] [b]   (middle empty kept)
IFS=,;  builtin printf "[%s] " $(printf "a,")    → [a]     (trailing delimiter elided)
IFS=" ,"; builtin printf "[%s] " $(printf "a, b, ")  → [a] [b]  (runs collapse, ends trim)
IFS=;   x="a b"; builtin printf "[%s] " $x  → [a b]        (`IFS=` disables)
```

The same splitter serves command arguments, the unquoted `$@`/`$*` re-split and
the `for … in` list (see "For-list words").

## Parameter expansion

`${name}` is the braced form of `$name` (an unset parameter expands to the empty
string, POSIX 2.6.2). Eight operators follow the name; the first operator byte
in the body decides which one, so `${v#a:}` is the pattern `a:` and `${v:-x#y}`
is the colon word `x#y`.

The colon family supplies its own word:

| form | result |
| --- | --- |
| `${name:-word}` | the word when `name` is unset or empty |
| `${name:=word}` | the word, assigned to the parameter `name` **points at** (with `!name`, its indirect target) and printed |
| `${name:+word}` | the word when `name` is set and non-empty |
| `${name:?word}` | `word` as the error message (`parameter null or not set` when the word is empty), rc 1 |

The pattern family removes an **anchored** match from the value: `${name#pat}`
strips the shortest matching prefix, `${name##pat}` the longest, `${name%pat}`
the shortest matching suffix, `${name%%pat}` the longest. No match leaves the
value whole (rc 0), and an empty pattern matches nothing, so it strips nothing.

```
v=abcabc    ${v#a}→bcabc  ${v#a*c}→abc  ${v##a*c}→(empty)  ${v%?}→abcab
p=/a/b/c.txt  ${p##*/}→c.txt  ${p%.*}→/a/b/c  ${p%%/*}→(empty)  ${p#*/}→a/b/c.txt
s=aXbXc     ${s#*X}→bXc  ${s##*X}→c  ${s%X*}→aXb  ${s%%X*}→a
```

`pat` is a *word* pattern, not a pathname glob: `*`, `?`, `[a-z]`, `[!a]`,
`[[:alpha:]]`, the escape pair `\X`, and quoted bytes as literals. It is never
expanded, there is no filesystem, so `set -f` (`noglob`) does not affect it, and
there is no `FNM_PERIOD` dot rule — a leading `*`/`?`/`[...]` may consume a
leading `.` (`d=.abc` with `${d#*c}` strips the whole value). A quoted pattern
byte is literal (`esc=a*c` with `${esc#"*"}` and `${esc#\*}` both print `a*c`).

`set -u` (`nounset`) bails an unbound parameter in the pattern family
(rc 1, `undef: unbound variable`), because a pattern has no word to fall back
on. The colon family stays exempt: its word supplies the value, so
`set -u; echo "${undef:-x}"` prints `x` and `${undef:-x#a}` prints `x#a`. An
indirect name takes the pattern form too: `${!ind#a}` strips the value that
`ind` names, and under `set -u` an unbound indirect name bails with bash's
`!p: unbound variable` / `nope: invalid indirect expansion` (rc 1).

The **name** may be a parameter rather than a variable (task #177): an ASCII
digit run is a positional parameter (`${1}` is `$1`, leading zeros allowed,
`${0}` is the program name) and a single-byte name among `#`, `?`, `$`, `@`,
`*`, `-` is a special parameter. The shared lookup resolves both for every
family, so the plain, colon and pattern arms all act on the parameter's value
and the nounset rule stays the single rule it is for variables: `set -u` bails
an out-of-range positional in the plain and pattern families
(`5: unbound variable`) and the colon family stays exempt. A digit run is
answered by the positional **alone**: the lookup never falls through to a
variable literally named `4`, so with `env 4=x` and `set -- a b c` both
`$4` and `${4}` are empty and `${4:-z}` prints `z` (bash's answer).

```
set -- a b c  ${1}→a  ${01}→a  ${0}→sh  ${1#c}→a  ${3%b}→c  ${2:-z}→b  ${4:+p}→(empty)
              ${?}→last status  ${$}→shell pid  ${@},${*}→positionals joined by the first IFS byte
              ${-}→fdshell's option flag bytes (`u` under `set -u`, empty by default)
              ${#}→`0`: in a word the length arm owns `${#…}` (task #173)
```

`:=` is the write arm, and a parameter has no slot to write into: `${5:=q}`
reports `5: cannot assign in this way` at rc 1 and creates no variable named
`5`, while `${1:=q}` prints the positional and writes nothing (bash's rule,
with its `$5: …` wording being #177's divergence).

`!name` is resolved with the colon operators as well (task #176), through the
same rule as the pattern family, and the value it points at is what the operator
acts on. Measured with `x=abcabc; v=x`:

```
${!v:-z}→abcabc  ${!v:+z}→z  ${!v:?m}→abcabc  ${!v:=q}→abcabc, x keeps abcabc
x=; v=x:  ${!v:-z}→z  ${!v:=q}→q, and x becomes q
v=y;:     ${!v:=q}→q, and the unset y is created with q
```

Two rules split the behaviour. The **nounset exemption is about a value**:
`set -u; ${!v:-z}` prints the target's value, and prints the word when the
target is unset — never a bail. An **unresolvable reference** is rc 1 in every
family, word or no word: a name that is itself unbound (`${!undef:-z}` →
`undef: invalid indirect expansion`) or bound to the empty string (`p=""` →
`: invalid indirect expansion`) names no target, and `:=` bails with it before
writing anything. `:=` writes the parameter the reference **points at** —
`${!v:=q}` assigns `x`, never a variable named `!v`. The bare `${!v+word}` form
is not an operator here; it belongs to the single-character family of the
divergence list above.

The stripped result is a normal word: it goes through field splitting and
pathname expansion afterwards, so `v=a*x` with `echo ${v#a}` prints the file `*x`
matches (`zx`), while a fully quoted word keeps the stripped text literal.

### The command word (word 0)

The command-name position is an expansion surface like any other word (task
#268): word 0 is expanded in the parent, before the function, intercept and
`PATH` lookups, so every dispatch path sees the same name — a plain command, a
pipeline stage, an `if`/`for`/`case` body, a function call, `command`/`builtin`,
`exec`, `become`, and `timeout`'s target word.

- `$( … )`, backticks, `$(( … ))`, `$name`/`${name}`, `%fd`/`%CWD` and a leading
  `~` all apply, whole-word and mid-word: `$(echo echo) hi` → `hi`,
  `$(echo "ec")ho hi` → `hi`, `cmd=echo; $cmd hi` → `hi`,
  `` `echo echo` hi `` → `hi`, `$((1+1))` looks up the command `2`,
  `~/bin/x` is `$HOME/bin/x`.
- Double quotes never suppress it (`"$(echo echo)" hi` → `hi`); single quotes
  keep the word literal, so `'$(echo echo)'` looks up the name `$(echo echo)`.
- The expansion is split on `IFS` and then pathname-expanded, and the fields are
  the words of the command: field 0 is the command name and fields 1.. are the
  leading arguments, arriving *before* the word's own arguments —
  `$(echo echo hi) x` runs `echo hi x`, and with `ea` (an executable), `eb` and
  `ed` in the directory, `./e* zz` runs `./ea` with `./eb ./ed zz`.
- Those leading fields are ordinary command-line arguments afterwards, so they
  take the global argv `%var` fd-substitution rule ("Passing file descriptors to
  subprocess"): with `%s` bound, `signalfd %s 10` followed by
  `$(echo ./ea; echo "$w")` with `w="%s"` runs `./ea` with the fd *number*
  (bash, which has no fd vars, passes the text `%s`), and the literal form is
  `\%var`.
- An unquoted word 0 whose expansion produces no field is not a word, so the
  next field becomes the command: `$(true) echo hi` → `hi`. A *quoted* empty
  expansion is a word, so the command name is empty: `"$(true)" echo hi` reports
  the name `""` (the same rule as "For-list words"), and `$(echo )` with nothing
  after it leaves no command at all.
- Escape pairs fold in word 0 (POSIX #4.1): `e\cho hi` runs `echo`. Inside
  double quotes the pair stays in the name (POSIX #4.2): `"e\cho"` is looked up
  literally.
- An expansion output field is protected from the parent's re-expansion — it
  reaches the command lookup encoded, so `$(echo '$HOME') x` looks up the
  literal name `$HOME` and `$(echo '`cmd`') x` the literal `` `cmd` ``.
- The substitution child's status is never the command's status, so
  `set -e; $(false) echo hi` prints `hi` and succeeds; over the capture limit
  `$(yes)` in word 0 is the usual capture-limit error.

Divergences from bash, all in the report, not the lookup: an empty command name
is fdshell's missing-command error at rc 1 where bash prints
`: command not found` at rc 127; a scoped `IFS` assignment is already in the
environment when word 0 is split (`IFS=: $(printf "echo:hi") zz` splits on `:`
and runs `echo hi zz`, where bash splits with the shell's `IFS` and reports the
name `echo:hi`); and `hash`'s name words stay literal (task #279).

### For-list words

Every word of a `for … in` list goes through the same pipeline a command
argument goes through (task #131), in bash's order:

1. braces are rewritten at text level, before tokenization (see "Brace
   expansion"), so they never re-expand the result of a substitution;
2. tilde, parameter, command, and arithmetic substitution and `%fd` / `$@`
   expansion run in-process;
3. the expansion is split on `IFS` bytes, driven by the word's own quote mask —
   a quoted byte never splits;
4. every field is pathname-expanded (`$(…)` output included), and each
   resulting field is one iteration of the loop.

```
x="a b";  for w in $x      → [a] [b]      IFS-split, each field globbed
x=a*b;    for w in "$x"    → the value, quoted bytes never glob
for w in ~                 → $HOME
for w in 1$((1+2))         → 13
for w in $(echo a*)        → the files matching `a*`
for w in `echo "*"`        → the files matching `*` (backtick fields glob too)
for w in "a$(printf " b")c" → [a bc]     (one word, substituted)
for w in "`echo a b`"      → [a b]       (a quoted substitution is one word)
for w in `echo a b`        → [a] [b]     (unquoted: the output is IFS-split)
```

A backtick word goes through the argument pipeline like any other (task #216),
so it follows the same split rule as every other expansion (task #132): a
**fully quoted** substitution is exactly one iteration — its spaces, tabs and
newlines are data and `IFS` never applies — and an unquoted one is split by the
same `IFS` splitter, non-whitespace `IFS` bytes included, with `IFS=` disabling
the split. Its fields are pathname-expanded like any other expansion's
(task #220).

The quoted-empty rule is the argument rule: `for w in "$(true)"` is one empty
iteration, `for w in $(true)` is none, `for w in "" x` iterates over `""` then
`x`. A list word that performed no expansion is one iteration:
`IFS=,; for w in a,b` iterates once over `a,b` (the splitting rule of
"Word splitting (`IFS`)"). A `%name` word expands to the fd number when `name`
is a scalar fd variable; when it is an fd array (`for %x in %arr`) the list
keeps the array reference and the loop binds an fd variable per entry. A
word's provenance is kept for `explain NAME`: a leading `~` reports `HOME`, a
word carrying a substitution reports command output, a literal word reports the
script.

Accepted divergence: a `&&` / `||` written in the `in` list is a list **word**,
not a cond-list operator — `for i in a && b; do echo hi; break; done` iterates
over `a`, `&&`, `b` (rc 0, printing `hi`), where bash rejects the line as a
syntax error (rc 2). It became reachable when task #137 made a whole
`for … do … done` block one cond-list part (see "Group command"); pinned by
`tests/cond_blocks.rs::for_list_with_and_is_an_accepted_divergence`.

### Nested regions inside double quotes

A `$( … )`, `${ … }` or `` ` … ` `` written **inside double quotes** is scanned as
one unit, everywhere the shell scans bytes (statement and cond-list splitting,
pipelines, redirects, heredoc operators, block scanning, the REPL's
completeness check): the region's own `"` are markers of the *inner* region and
never close the outer quote, its `;`, `|`, `&&`, `<`/`>` and `<<` are data, and
its bytes are marked quoted, so its expansion is one word and is never split on
`IFS`. Quote bytes inside the region are kept verbatim in a `$( … )` body (the
body is re-parsed by the substitution) and dropped in a `${ … }` or backtick body
(taken verbatim by the expansion):

```
echo "$(echo "a; b")"                        → a; b        (`;` is region data)
echo "$(printf "x && y")" && echo ok         → x && y, ok  (the inner `&&` is not an operator)
echo "$(printf "a | b")"                     → a | b       (no pipeline)
echo "$(printf "a > /dev/null")"             → a > /dev/null
echo "$(printf "<<EOF")"                     → <<EOF       (no heredoc opens)
builtin printf "[%s]" "$(printf "a b")"      → [a b]       (one word)
"$(printf "a b")" in `"a$(printf " b")c"`    → a bc        (text around it stays in the word)
echo "${u:-"z w"}"                           → z w         (quotes dropped, spaces kept)
echo "$(echo "$(printf "x && y")")"          → x && y      (region inside a region)
echo "`echo hi`"                             → hi          (a quoted backtick region expands)
echo "`printf "q r"`"                        → q           (expanded; the body's quotes drop — see below)
echo "$(echo `echo a`)"                      → a           (a backtick inside a $( … ) body)
```

An unterminated region is a clean parse error whose caret is on the *outer*
quote (the enclosing quote is what stays open); `echo "$"`, `echo "${"` and
`echo "${x"` keep printing `$`, `${` and `${x` literally.

A backtick region is expanded wherever the argument pipeline runs (command
arguments, assignments, `local`/`let`, `set --`, here-strings, `case` words,
heredoc bodies with an unquoted delimiter, and `for … in` lists) — task #216:
the region runs through the same `cmd_subst` machinery as `$( … )`, and the
output takes its split protection from the region's quote mask, so a quoted
region is one word and an unquoted one splits on `IFS`.

Accepted divergences (scanning is fixed, expansion is not):

- quotes written **inside** a quoted backtick body are markers, not content:
  the region scanner drops them, so the body's quoted bytes reach the child
  without their protection (the same rule as a `${ … }` body) —
  `echo "`printf "q r"`"` prints `q` where bash prints `q r`, because the child
  sees `printf q r`. Pairing is a naive left-to-right match on the text after
  quote removal, where bash pairs raw bytes; and a backtick body nests no
  `$( … )` at scan level (see the Limitations under "Heredocs");
- the word-0 (command-name) and redirect-target positions are never
  substituted, so a backtick there is a literal (task #268);
- a `$( … )` nested inside a `${ … }` body is scanned as a unit and stays
  literal (`echo "x${u:-"$(echo "p q")"}y"` → `x$(echo p q)y`); the `${ … }` body
  is taken verbatim (task #174);
- `IFS=,; printf "[%s]" x$v,y` (with `v=$(echo "b c")`) yields two words here
  and one in bash: fdshell splits the whole word — the literal part included —
  on `IFS`, bash splits only the expansion part, so a literal `IFS` byte placed
  after an unquoted embedded expansion cuts between the expansion's last field
  and the literal text (task #222).

Other accepted divergences of the `for … in` word pipeline (task #131):

- `"$@"` with no positionals is **one empty iteration** here, where bash gives
  none (the quoted-empty rule of `substitute_args`; task #219);
- `~` and `~/…` expand to `$HOME`, `~user` stays literal (bash resolves it via
  the password database);
- `set -u` with an unbound word (`for w in $undef`) and a `${zz:?}` word bail
  the statement at rc 1 with the same messages as everywhere else in fdshell;
  bash's `-c` mode exits 127 (LESSONS.md accepts the rc asymmetry).

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

The substitution has no percent escaping of its own (task #155): `%%` is passed
through untouched, so a builtin that reads `%%` as its own escape — `printf
'%%'` printing `%` — keeps working, and a literal `%` in front of a bound fd
name is written `\%var` outside quotes.

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

### `time`

`time [-p] <statement>` runs the statement and reports its real, user and sys times on
**stderr**, keeping the statement's own exit status:

```console
$ fdshell -c 'time sleep 0.2'

real	0m0.204s
user	0m0.003s
sys	0m0.000s
$ fdshell -c 'time -p sleep 1.2'
real 1.20
user 0.01
sys 0.01
$ fdshell -c 'time false; echo $?'
1
```

The default layout is bash's (a leading blank line, tab-separated `N m N.NNN s` fields);
`-p` selects the POSIX layout (three `name value` lines, values in seconds). The report
inherits the shell's stdout/stderr, so the inner's own output is untouched.

Real time is `CLOCK_MONOTONIC` elapsed while the statement ran; user/sys are CPU times,
measured with `getrusage(RUSAGE_SELF)` before and after the statement plus the `rusage`
of the children reaped inside it — so a forked command's CPU (and a `wait`ed background
job's) is counted. The report covers exactly the timed statement: no fork happens for
builtins, user functions, `if`/`while`/`for`/`case` blocks, groups, pipelines,
assignments, loops, or control flow, and the statement's status and control flow
(`break`/`continue`/`return`) pass through the report untouched.

`time` is a reserved word, so it is recognized as the first word of a statement with
**none of its characters quoted** (`"time" ls`, `t"i"me ls`, `\time ls`, `'time' ls` are
the `/usr/bin/time` utility, as in bash). Every leading `time [-p]`
prefix collapses into one statement — `time time -p ls` and `time -p time -p ls` each
print a single report, the second in the POSIX layout — and the prefix run ends at the
first word that is not a `time` with its own unquoted `-p`. A `-p` written after the
statement is one of its arguments, and so is a quoted one: `time -p -p ls` and
`time "-p" ls` both try to run the command `-p` (command-not-found), exactly as bash does.
A `time` alias is not expanded: `time` is in the reserved-word list the alias expander
skips, exactly like `if` or `done`.

A statement that ends the shell (`exit`, `quit`, `exec`, `become`) prints its report
*before* it runs, because nothing runs after it (`time exit 3` reports, then exits 3).
`exit` inside a pipeline (`exit | cat`) runs in a child, so the report is the pipeline's
own. `.`/`source` is **not** a terminating statement: the script runs in the current
shell and returns, so `time -p . file` reports *after* the sourced script, covering
everything the script did.

Limitations / deviations from bash:

* `time && false` runs the bare `time` (a zero report) and then `false`. fdshell splits
  cond-lists before parsing, so `time` is left with no statement; bash raises a syntax
  error there.
* An expansion-produced `time` (`$(echo time) ls`) *is* timed: word 0 is expanded
  before the dispatch, so the `time` handler sees the name bash's re-scan produces
  (task #268). Only the *reserved word* is parse-time: `time` written in a
  `$( … )` body is not a keyword for the parse.
* `TIMEFORMAT` is ignored: the report is always one of the two layouts above.
* A background job started inside the timed statement (`cmd &>&job`) is not reaped at
  report time, so its CPU is missing from `user`/`sys` (their contribution is ≈0). The
  `times` builtin accumulates it as usual once `wait`/`waitpid` reaps the job.
* `user`/`sys` are the shell's own CPU around the statement plus the CPU of the children
  it reaped, because the statement runs in the current shell; bash times a forked
  subshell. The numbers are comparable in magnitude, not identical.

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
  the stale path is `EADDRINUSE` (98) on the next bind; `--addr ADDR --port N`
  is AF_INET v4 (`--bind ADDR` is accepted as the historical spelling of the
  same flag). Names and paths are at most 107 bytes.
* `--backlog N` belongs to `listen` alone — it is the only builtin that calls
  `listen(2)`, so `bind` and `connect` reject the flag (`invalid argument
  backlog`) instead of silently ignoring it.
* Output uses the usual capture forms: `builtin listen @srv %>%l`, or the
  bounded form `builtin accept %l %>%conns[8]` to fill an fd array. At the
  cap, further connections are accepted and closed immediately.
* `accept` blocks until a connection arrives; kernel failures (e.g. `listen`
  on a dgram socket) surface as their errno exit code. Without a capture,
  `accept` accepts and closes immediately (the `pipe` precedent).
* `connect` is the client side: it creates the socket (default `stream`) and
  connects it to the ADDRESS — for `connect` the `--addr ADDR --port N` pair
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


### Broken pipes (`SIGPIPE`)

Every program fdshell hands off to runs with `SIGPIPE` at its default disposition and unblocked:
that is every forked child (pipeline stages, launched commands, `$(…)` bodies, `wait` arms) and
every program that replaces the shell's image through `exec`/`become`.

A write to a pipe that has no readers left therefore kills the writer *silently*, exactly as a
shell stage should:

```sh
seq 1000000 | echo got   # prints got, nothing on stderr
echo hi | echo got       # prints got, nothing on stderr
```

The killed stage reports status `141` (`128 + 13`), and the pipeline's status is the last stage's,
so `seq 1000000 | cat` with a closed stdout exits `141` and `seq 1000000 | echo got` exits `0`.
`exec seq 1000000` with a closed stdout leaves the process signaled by `SIGPIPE` (bash does the
same), because after the hand-off the process *is* `seq`.

`signalfd %v PIPE` blocks `SIGPIPE` in the shell process — that is how the signalfd receives it —
and the shell clears the block for every child it forks.

The shell process itself keeps `SIGPIPE` ignored (Rust's std installs `SIG_IGN` at startup, and
`bash -c` has `SigIgn=0`): fdshell's own writes to a closed stdout do not kill the shell. This is
deliberate — an interactive shell must survive a closed stdout — and it is the only place where
fdshell diverges from bash here.


### Implementation philosophy

The fdshell binary is a dynamically linked Linux x86_64 binary (glibc + libgcc_s). It has no dependencies beyond the OS kernel APIs it calls directly via syscalls.

Currently only Linux is supported and only x86_64.

The project is a workspace with three crates:

* `safe/fdshell` - main shell logic, spawning and receiving file descriptors
* `safe/builtins` - builtin commands (no_std, forbid(unsafe))
* `unsafe/sys` - syscall wrappers (no_std, unsafe allowed)

All `safe/` crates have `forbid(unsafe_code)` and cannot call libc directly. Non-test source files should aim for ≤90 lines, though a few exceed this limit (e.g. `debug.rs`, `caret.rs`, `error/parse.rs`).

The codebase uses `#[derive]` where idiomatic — commonly `Clone`, `Default`, `Debug`, `PartialEq`, and `Display`. Error types derive `Display` + `Debug`. The shell crate (`safe/fdshell`) is `#![no_std]` with `extern crate std` for runtime stability.
