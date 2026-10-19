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
- `&>` and `&>>` redirect stdout and stderr together (a bash extension). Like
  bash they open the operand **once** and share that open file description
  between fd 1 and fd 2 (bash's `> f 2>&1`), so the streams append to each
  other; `cmd > f 2> f` stays two opens. In POSIX, `cmd &> file` is a
  background command followed by a truncating redirection.
- Divergence within the family: the `&>` def's fd-2 half is applied in fd 1's
  slot of the command's redirection table, so a later fd-2 redirection wins it.
  `cmd 2>&- &> f` fails (rc 1, fd 2 closed) and `cmd 2>&1 &> f` sends fd 2 to
  the **pre-redirect** fd 1, where bash applies `&>` later and reopens fd 2
  onto `f`. Pinned with the measured values in
  `safe/fdshell/tests/redirect.rs`; owned by task #337.

## Operators are recognized at word starts

In POSIX, a redirection operator is recognized wherever it appears outside
quotes, even glued to a word. fdshell never splits a word at an operator
byte, so:

```shell
echo hi>file        # fdshell: the argument `hi>file`; POSIX: write `hi` to `file`
echo hi >file       # both: write `hi` to `file`
```

Write a space (or a quote) between a word and an operator that follows it.

The same raw-byte rule decides the operand side: an operand of a bare `<<`,
`<<<`, `>`, `>>`, `<`, `<>`, `>|`, `&>` or `&>>` is an operator (and a syntax
error) only when its leading `<`, `>`, `&` or `%` byte is **unquoted**.
POSIX removes quotes after tokenization (#4.1), so a quoted byte is ordinary
word text and bash opens the word as a file — fdshell matches it:
`> "&f"` opens `&f`, `cat <<< "<x"` is the word `<x`, while `> &"f"` keeps
its `&` unquoted and stays a syntax error in both shells. The one
intentional exception is the `%`-namespace arm: the *spaced* fd-variable
form is never a plain file operand (`> %v` is fdshell's syntax error where
POSIX creates a file named `%v`) — the fd-variable targets are the attached
`>%var` and `&>%var` forms of the section above.

## Here-document and here-string descriptor numbers

`[N]<<word` delivers the body to descriptor `N` (`cat 3<<EOF` writes the body
to fd 3 and leaves stdin untouched) and `[N]<<<word` delivers the expanded word
plus a newline the same way (`cat 3<<<text`). fdshell reads `N` with the same
rule as `N>file`, on raw bytes, and that gives three differences from bash:

- The digits must begin the operator's word unescaped: a quoted prefix, a
  quoted operator, an escaped digit, or digits glued to a longer word are word
  data — fdshell passes the whole token to the command (`cat "3"<<EOF` gets
  the argument `3<<EOF`, its quotes taken as ordinary word quoting and its
  `<<` as plain word text). bash never reads a descriptor number from these
  shapes either, but it splits the word at the operator (`cat "3"<<EOF` gets
  the argument `3`) and then reads the body lines as the here-document;
  fdshell never splits (the word rule above) and the body lines are commands.
  The `<<<` form has the same word rule: `cat "3"<<<x`, `cat 3"<<<"x`,
  `cat \3<<<x` and `cat x3<<<y` are one argument each.
- The number is validated as a descriptor, so a run that does not fit is a
  parse error naming it: `cat 99999999999999999999999<<EOF` reports `is not a
  valid descriptor number` where bash passes `999…` as an argument. Same for
  `cat 99999999999999999999999<<<x`, which reports `here-string: '999…' is not
  a valid descriptor number`.
- A number-prefixed operator at the start of a run is fdshell's command word
  (`3<<EOF` and `3<<<x` alone run as commands, rc 1); bash opens fd 3 for the
  here-document (and prints nothing) and reports `3: Bad file descriptor` for
  the here-string.

The `<<<` form's remaining differences from bash:

- `cat 3<<<<<x` is the here-string `<<x` on fd 3 (the pinned `cat <<<<<x`
  shape, fd-ified); bash rejects it as a syntax error (rc 2).
- A bare `3<<<` at end of input takes the empty word, printing a lone newline
  to fd 3; bash rejects it as a syntax error (rc 2).
- An expansion failure in the word is an error (`cat 3<<<$99999999999999999999`
  reports `index or value too large`); bash expands it to the empty word and
  prints a newline.
- A sibling `>&N`/`<&N` dup of the same command cannot read the descriptor the
  `N<<<word` form opens: fdshell reports `fd 3 is not open` where bash prints
  the body (`cat 3<<<text <&3`). Owning task: #178/#183 (order of redirections).
- A redirect written after a group's `}` is not attached to the group
  (`{ cat; } 3<<<x` is a parse error); owning tasks: #187/#191.
- `cat 3<<<x &` is a parse error because `&` background is one for every
  command; owning task: #313 family.

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

## Quoted keywords are command words, quoted control keywords are not

POSIX §2.9.1 recognizes a reserved word only when none of its characters is
quoted, and fdshell follows it at every layer of the parse: a word whose raw
span carries a quote byte is data, so `'if' x`, `"while" x`, `i"f" x`, `""case
x`, `if true; ''then echo a; fi` and `f'oo'() { :; }` are command words and
syntax errors exactly as bash makes them.

The one accepted divergence is the family of keywords fdshell implements
without a builtin of that name: bash runs `'break' 1`, `'continue' 1`,
`'return' 3`, `'umask' 022` and `'unset' zz` as its **builtins** (the quoted
word is looked up, and the builtin answers), where fdshell keeps the keyword
and answers with its own message (`'break' is not inside a loop`, rc 1) or its
own parse rule (`'unset' zz`: `variable must start with '%'`). Gating those
words on §2.9.1 would trade a real builtin run for `break: not found`, so
fdshell gates only the block layer and leaves these keywords quoted-blind
(task #335 owns the decision, together with the builtins it needs).

A second divergence is mechanical: fdshell parses a script **statement by
statement**, where bash parses the whole input before running any of it, so a
quoted opener runs as a command and the rest of the input keeps executing:
`'while' :; do echo a; break; done` runs its statements one at a time
(`"while" not found`, `"do" not found`, `'break' is not inside a loop`) and
prints nothing, rc 1, where bash parses the whole input first and aborts with
rc 2. The same rule decides a **multi-line** quoted definer: when the body of
`function 'f' { … }` (or `f'oo'() { … }`) is spread over several lines, the
brace becomes a group statement and its body runs — fdshell prints `hi` with
rc 1, while bash reports `'f'`: not a valid identifier and prints nothing.
Written on one line the body never runs, so those forms agree on stdout.

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
