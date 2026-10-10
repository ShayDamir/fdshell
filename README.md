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
sources; see below. A command may carry several redirections to the same fd;
all of them are applied in the order written, so the **last one to that fd
wins** and it is not an error: `echo hi >a >b` writes `hi` to `b` and creates
`a` empty (the first redirect opened and truncated its target), and
`cat <f1 <f2` reads `f2`. A failing redirection aborts the command, so the
order matters: `cat <missing <f2` exits 1 without reading `f2`.

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
- within one command `> a 2>&1` copies the fd 1 the shell had **before** the
  command: every redirect source is resolved up front, so `2>&1` cannot see the
  `> a` target. Bash applies as it goes and `2>&1` lands on the file `a`
  (tracked by task #183).
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
- The `$_` arm is not nounset-checked (unobservable in `-c` mode: `set -u`
  binds `_` exactly as bash does, so `set -u; builtin echo "[$_]"` prints
  `[-u]` on both).
- Arithmetic variables are not nounset-checked: `set -u; echo $((unsetv + 1))`
  prints `1` here, where bash exits with `unsetv: unbound variable` (tracked by
  task #170).
- A `%name` fd variable that is unbound stays literal (`%nosuchfd` prints
  `%nosuchfd`). The fd namespace is a fdshell extension, not a POSIX parameter.
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
- The bare-assignment path expands its value with no quote mask, so a quoted
  `[` in `${x#"["a]}` is treated as unquoted there and strips; bash keeps it
  literal.
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
- `${!name}` inside the **colon** operators is not resolved (bash `${!v:-z}`
  prints the indirect target's value; fdshell prints the word `z`) — task #176.
- A positional parameter is not a braced name: `${1}` and `${1#a}` expand to
  empty; bash gives the positional `1` (task #177).

## Parameter expansion

`${name}` is the braced form of `$name` (an unset parameter expands to the empty
string, POSIX 2.6.2). Eight operators follow the name; the first operator byte
in the body decides which one, so `${v#a:}` is the pattern `a:` and `${v:-x#y}`
is the colon word `x#y`.

The colon family supplies its own word:

| form | result |
| --- | --- |
| `${name:-word}` | the word when `name` is unset or empty |
| `${name:=word}` | the word, assigned to `name` (and printed) |
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

The stripped result is a normal word: it goes through field splitting and
pathname expansion afterwards, so `v=a*x` with `echo ${v#a}` prints the file `*x`
matches (`zx`), while a fully quoted word keeps the stripped text literal.

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
