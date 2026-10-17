# POSIX compatibility

fdshell implements the POSIX shell language: word splitting, quoting,
parameter expansion, command substitution, globbing, arithmetic, control
flow and functions behave as POSIX describes them, and the builtin utilities
follow their POSIX interfaces.

This page lists the **deliberate, permanent incompatibilities** — the places
where fdshell will never match POSIX because matching it would remove one of
the shell's core features. Gaps that are merely not implemented yet are bugs,
not design, and are not documented here.

## `%` is a file descriptor namespace

POSIX gives `%` no special meaning in a word. fdshell binds file descriptors
to names and expands them in words, arguments and redirection targets:

- `%name` expands to the number of the descriptor bound to `name`; an unbound
  `%name` stays literal text.
- A literal `%` immediately before the name of a bound descriptor is written
  `\%` (outside quotes). `%%` is **not** an escape: both bytes reach the
  command, so `printf '%%'` still prints `%`.
- A capture word `cmd %>%var` is not passed to `cmd` as an argument: it
  stores the descriptor `cmd` returns in `%var`. `%tag>%var` and
  `%>%arr[N]` are the tagged and array forms.
- `>|%var` and `&>%var` redirect to the descriptor, where POSIX would treat
  `%var` as a file name.
- `%CWD` and `%?` are special names of the same namespace.

A portable script is affected only while an fd variable is bound, but then a
plain word such as `printf %s` is rewritten to a descriptor number if `%s`
is bound.

## `&` names background tasks

POSIX uses `&` as the background control operator and never as part of a
word. fdshell additionally reads `&name` as a task variable:

- `cmd &>&name` starts `cmd` in the background and stores the task (a pidfd
  plus its capture context) under `&name`; `&>|&name` overwrites an existing
  task.
- `waitpid &name` waits for that task.
- `&>` and `&>>` redirect stdout and stderr together (a bash extension). In
  POSIX, `cmd &> file` is a background command followed by a truncating
  redirection.

## Operators are recognized at word starts

In POSIX, a redirection operator is recognized wherever it appears outside
quotes, even glued to a word. fdshell never splits a word at an operator
byte, so:

```shell
echo hi>file        # fdshell: the argument `hi>file`; POSIX: write `hi` to `file`
echo hi >file       # both: write `hi` to `file`
```

Write a space (or a quote) between a word and an operator that follows it.

## `wait` is also a block keyword

POSIX `wait` waits for background jobs. fdshell opens an event block when
`wait` is followed by a pattern arm, including on a following line:

```shell
wait
    readable %rd) …
done
```

A bare `wait` at the end of a line inside a multi-line script therefore
starts a block where POSIX would run the builtin with no arguments; then the
lines after it are parsed as arms. Writing `wait;` (or `wait $!`) keeps the
POSIX meaning.

## `-c` mode keeps its own `$0`

POSIX: `sh -c 'echo $0' name` prints `name`. fdshell sets `$0` to the
invocation name and keeps the operands as `$1..`, so
`sh -c 'echo $0 $1' a b` prints `sh a`, not `a b`. A script file still sees
its own path in `$0` as POSIX requires.

## Strict mode

`shopt -s strict` is an opt-in FDSHELL extension: it bans absolute paths and
implicit-CWD resolution for the `*at` builtins, redirect targets and program
words, requiring every operation to be pinned to an explicit descriptor. A
portable script that relies on absolute paths must not enable it. Without
`strict`, path handling follows POSIX.
