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
- Quoting the whole word quotes the operator: `cmd '&>&name'` is a plain
  argument in both shells (`echo hi '&>&name'` prints `hi &>&name` and launches
  no task, rc 0). The **unquoted** form is fdshell's extension — bash rejects
  `echo hi &>&name` with rc 2 (`syntax error near unexpected token '&'`),
  fdshell records the task and prints `hi`.
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

POSIX also uses a trailing `&` as the async control operator (POSIX
#3.9.4.1: `echo hi &` runs `echo hi` asynchronously). fdshell supports it:
the statement before the `&` runs as a **nameless background job** — the shell
forks itself, the child runs the statement through the identical foreground
path and exits with its status, the parent records the child as a job `$!`
reports and `wait` / `wait <pid>` reap, and continues with status 0.
`safe/fdshell/tests/async.rs` pins the family.

- The `&` is the operator only at a word's start: `a&b` stays one word (the
  word rule above), `"&"` and `\&` are data, `&&`'s second byte, `N>&N`,
  `|&`, `<&N` and `(&`/`)&` keep their meanings, and a `>`/`<` written before
  it across spaces owns the byte — bash tokenizes `>&` as one operator, so
  `cat > &f` is a redirect (fdshell's "invalid redirect"), never a split.
- bash's preflight property is kept: an invalid split — nothing to terminate
  (`& echo b`), a `;`-terminated target (`echo a ; & b`), or a `;`/`|`/`&`
  peeked behind the `&` (`echo a & ; echo b`, `echo a & | cat`,
  `echo a & & b`) — is a parse error (rc 1) that runs **nothing**.
- `wait` with no operands returns 0 whatever the reaped jobs' statuses were
  (POSIX 2.14; `false & wait; echo $?` → `0`, bash 5.3.9 measured identical).
- Measured divergences, all pinned: a top-level **compound** with a trailing
  `&` (`if :; then echo x; fi & echo y`, `time cmd &`) and an
  **assignment-only** statement (`x=1 &`) run **foreground** — only `Cmd` and
  `Pipeline` become async jobs; bash backgrounds the whole thing as one job
  (owners #352, #353). `echo a && echo b &` is two jobs where bash makes one
  (identical output set and rc). A `%>%name` capture on a backgrounded command
  commits in the discarded child, so the parent never sees the fd variable —
  the named capture form for background stays `cmd &>&name`. `$!` for a
  backgrounded pipeline is the job's shell pid, not the last stage's.
  `echo a &` followed by a `;` line (`echo a &\n; echo b`) streams: line 1
  executes and the `;` line is fdshell's accepted empty statement, where bash
  stops with rc 2 (the same-line `& ;` form matches bash).
- A plain `&` written **inside a block's text** is a separator inside the span
  (`for i in 1; do echo hi & done`, `{ echo hi & }`, `f(){ echo hi & }`,
  `case x in x) echo a & ;; esac`, `if true & then echo x; fi`): the block stays
  one segment, its closer may sit right behind the `&` on the same line, and the
  statement in front of the `&` runs as a background job **in the body** (the
  body's job list is the shell's, `$!` and `wait` see it). At the parse layer the
  word-start `&` is a command-list terminator, so each closer glued after it
  (`& done`, `& }`, `& fi`, `& ;;`, `& esac`) is found, and the body slice
  **keeps** the `&` — the body's own scan is what marks the flag. A `;` glued
  right behind the `&` is **kept in the body too** (`for i in 1; do echo a & ;
  done`): the body's own scan then reports the split bash calls a syntax
  error — rc 1, nothing runs (bash rc 2) — in every body, arm, condition and
  function-def position; the multi-line `&` newline `;` form streams as bash
  runs line 1 (the `;` line is fdshell's accepted divergence), and an
  **untaken** arm's body is parsed, never scanned (`if true; then :; else
  echo a & ; fi` runs nothing at rc 0). `for`'s word list is not statement
  text and stays lenient: `for i in 1 & do :; done` is a parse error (bash rc
  2, fdshell rc 1), and `for i in 1 & ; do echo x; done` runs the `&` as a
  word (bash rc 2, fdshell rc 0 — pinned). A `&`-terminated **condition**
  (`if true & then echo x; fi`, `while : & do …; break; done`, the `elif` twin)
  is a background job of its own: **one nameless job per condition run**, so a
  loop records one per iteration, `$!` reports it and `wait` reaps it. The
  condition slice **cuts** that `&` (the cut feeds the here-doc body indexing
  and the `& ;` scan, and the parse layer rejects a bare `&`), so the fact
  travels as the parsed block's `cond_background` flag into the conditional
  list's background argument. The arm/loop it gates sees the **async status 0**
  — `if false & then echo x; else echo y; fi` prints `x`,
  `while false & do echo x; break; done` prints `x`,
  `until false & do echo x; break; done` prints nothing — and the condition's
  side effects die with the child (`if cd /tmp & then :; fi` leaves the cwd
  alone, `if exit &` leaves the script running). The `&` **newline** form is
  the same shape (`if true &` newline `then echo x; fi` is a job), and
  `if true && false & then echo x; fi` prints `x` like bash. Measured
  divergences of the condition family, all pinned: a condition whose last part
  is **short-circuited away** records no job and gates on that part's status
  (`if false && true & then echo x; fi` prints nothing where bash prints `x`,
  and `if cd a && cd a &` cds in the parent) and a **compound** last part
  (`{ … } &`, `for … done &` written as a condition) runs foreground — bash
  backgrounds the whole list/compound, owner **#352**; a `&` on its **own
  line** behind the condition is the deferral family, owner **#313**; an
  **empty** condition (`if & then`, `while & do`) is a parse error since task
  **#299** (bash rc 2, rc 1 here), so it has no job to record; `break`/`continue`/`return` written with the
  `&` reach the **parent** (the loop-control keyword is not a `Cmd`), owner
  **#362**; `{ echo <<T & }` plus body hits the
   **single-line group** here-doc gap of task **#315** (the same shape without
   the `&`, `{ cat <<T; }`, reports it identically; the multi-line group body is
   fine). The rule does not loosen the statement-level guard:

  `echo a & ;; echo b` **outside** a block is bash's syntax error, rc 1. Pinned
  in `safe/fdshell/tests/async.rs`, `loops.rs`, `group.rs`, `case.rs`,
  `cond_blocks.rs`, `block_bodies.rs`, `block_conditions.rs`, `repl.rs` and the
  `segment`/`parse` unit tests.
- A **here-doc** travels with the statement its `&` terminates, and a second
  here-doc written **behind** the `&` on the same line is a separate statement
  with its own body: `cat <<A & cat <<B` plus the four body lines prints both
  bodies with rc 0 in both shells (fdshell printed `x` and then ran `y`/`B` as
  commands). The line's body regions are one stream, allocated in operator
  order, so the prefix takes region 0 and the suffix region 1. The shape
  composes with the io-number form (`cat 3<<B` behind the `&`), `<<'B'`, `<<-B`,
  a pipeline suffix (`cat <<B | wc -l`), several `&` on one line, and every
  block/arm position (`if … fi`, `{ … }`, `f(){ … }`, `for … done`,
  `case … esac`). Measured divergences, all pinned:
  a **redirect-only** suffix behind the `&` (`cat <<A & <<B`, `echo hi <<T & <<U`)
  runs `<<B` as a command word (bash runs a bare redirection, rc 0) — owners
  **#303** and **#345**; the glued `cat <<A& cat <<B` keeps the tokenizer's
  word rule (`&` is an ordinary word byte, so the byte-level delimiter is `A&`)
   and reports the missing terminator; a `#` comment behind the `&` ends the
   physical line, so the suffix run is flushed body-less (the rule the
   comment-terminated line has always had, and the deferred-flush member task
   **#333** owns); a bare `<<` behind the `&` is

  `invalid redirect syntax` rc 1 where bash is rc 2; a body that never arrives
  empties the **whole line's** region list (owner **#275**); a **block opened
  behind the `&`** (`cat <<A & if true; then cat <<B … fi`) prints the prefix's
  body lines twice, since the block's span has to cover them — the one-job async
  list family **#352**/**#316**. Async output has no order, so the pins give each
  writer its own file. Pinned in `safe/fdshell/tests/async.rs`, `heredoc.rs`,
  `repl.rs` and the `segment` unit tests.
- **Line deferral** (the accepted divergence behind those multi-line shapes):
  a line whose first non-blank byte is `&` (indentation counts: `echo a\n & echo b`
  is the same shape) claims the statement buffered above
  it, so `echo a\n& echo b` is `echo a` as **one background job** plus
  `echo b` in the foreground (both print, rc 0), and `echo a &\n& echo b`
  marks the same statement background twice over — where bash rejects the
  line-opening `&` with rc 2. fdshell scans each line as its own statement
  text, so the flush of a line is deferred exactly to give that `&` a target;
  the deferral is also what lets `echo a &\necho b` run both statements
  bash-like. Pinned in `safe/fdshell/tests/async.rs` and in
  `safe/fdshell/src/segment/bg/tests.rs`.

## Operators are recognized at word starts

In POSIX, a redirection operator is recognized wherever it appears outside
quotes, even glued to a word. fdshell never splits a word at an operator
byte, so:

```shell
echo hi>file        # fdshell: the argument `hi>file`; POSIX: write `hi` to `file`
echo hi >file       # both: write `hi` to `file`
```

Write a space (or a quote) between a word and an operator that follows it.

A **keyword block's closing keyword** is the one place the glue is read the way
bash reads it: a closer glued to a redirection operator is recognized as the
closer, and the operator redirects the whole block — `while :; do :; done>f`,
`if :; then :; fi>f`, `case x in x) :;; esac>f` all file the block's output in
`f`, as bash does. The keyword side of a word start is a boundary check, not a
word split, so nothing is split off; the *operand* side keeps the no-split rule,
so `done>f>g` opens the single file `f>g` where bash opens `f` and then `g`.

A **fully quoted** operator word is never an operator — the gate every fdshell
redirect parser opens with, the `&>` family included: `echo '&>f'` prints `&>f`
and opens nothing, `echo hi '&>' f` prints `hi &> f` and opens nothing, and
`cmd '&>&name'` is an argument that starts no background task. POSIX #4.1
removes quotes after tokenization, so bash does the same.

The same rule extends **into** the operator (POSIX XCU 2.9.1, task #264): a
redirection is built from unquoted io-number digits and unquoted operator
bytes, so a quote byte ends the operator there. fdshell gates its redirect
parsers on the token's quote mask, and these forms are bash parity:
`2'>'g`, `1">"f`, `1">|"f`, `'<'f`, `'>'f`, `'>>'f`, `'>|'f`, `'&>'f`,
`'&>>'f`, `'&>'%v`, `2'>&1'`, `2>"&"1` and `>"&"f` (the quoted `&` ends the
operator, so the *file* `&1` / `&f` opens), `1'>'` (an argument, not a parse
error), and a quoted io-number `'2'>f` — the operator keeps its default
descriptor (fd 1), the digit is no fd.

The surviving divergence is the **argument list**: bash *splits* a word at an
unquoted operator byte, fdshell never does, so bash's `echo x '1'>g` writes
`x 1` into `g` (the quoted `1` is an argument) where fdshell writes `x`, and
the same split keeps bash's `echo x "&">y` writing `y` = `x &`, `echo a
2\>>f`, `echo a \1>f`, `echo a 2&>f` and `cat '3'<<EOF` behaving as their
word rules dictate.

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
and answers with its own message (`'break' is not inside a loop`, rc 1).
`'unset' zz` is the row where the two coincide **by accident**: the quoted word
is still the keyword here, and since task #290 every word after `unset` is an
operand — a plain name is a shell variable, no operand is a parse error — so
fdshell removes `zz` at rc 0 exactly as the builtin does. Gating those
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
