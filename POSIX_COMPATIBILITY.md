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
- The fd-variable **target** is the byte right after the operator, and only
  while it is **unquoted**: quoting the `%` makes the target the POSIX
  file-name word — `> "%v"`, `> '%v'`, `>| "%v"`, `>> "%v"` and `&> "%v"`
  create/open the file `%v`, exactly as bash (task #329). The **word**
  layer is the twin where quoting is *not* an escape: a **bound** `%name`
  expands inside quotes (`echo "%name"` prints the fd number, bash prints
  `%name`), `\%` is the only escape, and gating that arm on the mask too is
  task #252's open decision.
- The dup word `>&%var` is deliberately **mask-blind**: bash answers
  `ambiguous redirect` rc 1 for `>&%v` and `>&"%v"` alike — a `>&word`
  operand is never a file name — so no quoting rule applies to it and
  fdshell keeps the fd lookup in both spellings.
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
- Redirection **order** follows POSIX #2.5: a command's redirections are
  applied one by one in the order they are written, and each sees the
  descriptors the earlier ones left; the target opens run in that order too and
  stop at the first failure. So `cmd > f 2>&1` sends fd 2 into `f`, `cmd 2>&1
  > f` leaves fd 2 on the terminal, `cmd 2>&- &> f` reopens fd 2 onto `f`, and
  `cmd 2> missing/dir/x > f` creates no `f` — all measured against bash 5.3.9
  and pinned in `safe/fdshell/tests/redirect.rs`.

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
  **#362**; `{ cat <<T & }` plus body prints the body at rc 0 and
   `{ echo <<T & }` plus body prints nothing at rc 0 — the `&` job of a
   single-line group body rides the region the group claims, at parity since
   task **#315** closed the group body's gap (the `cat` row pinned in
   `tests/async.rs::a_heredoc_behind_an_ampersand_inside_a_single_line_group_body`;
   `echo`'s empty line lands in the job's stdout, so it is measured, not pinned).
   The rule does not loosen the statement-level guard:

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
  runs `<<B` as a command word (bash runs a bare redirection, rc 0) — the
  here-doc half, owner **#345** (a statement with a here-doc body keeps the
  operator-word behavior; the plain redirect-only statement of #303 never
  reaches it, since its body region is what gates the parse); the glued
  `cat <<A& cat <<B` keeps the tokenizer's
  word rule (`&` is an ordinary word byte, so the byte-level delimiter is `A&`)
   and reports the missing terminator; a `#` comment behind the `&` ends the
   physical line, and a `#`-terminated line keeps the newline's whole
   end-of-line bookkeeping (task **#333** closed the body-less member of that
   family), so the suffix is flushed on its own line and claims the region its
   own operator owns: `cat <<A & cat <<B # note` plus the four body lines prints
   both bodies at rc 0 in both shells. A bare `<<` behind the `&` is

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

A **closing keyword closes its own kind of block** (task #309): `fi` an `if`,
`done` a `while`/`until`/`for`, `esac` a `case`, `}` a group. POSIX #2.9.1's
grammar keeps those words reserved, so a `fi` written inside a `while` body is a
syntax error there (rc 2). fdshell reads a **mismatched** closer as an ordinary
**command word**, so the body keeps the statement written behind it and runs:
`while :; do echo if; fi; echo x; break; done; echo after` prints `if`, `x` and
`after` and exits 0, reporting the stray closer as
`failed to resolve command path: "fi"` (rc 127 inside the body). Where the
mismatch consumes the block's only closer the line is fdshell's own parse error
— `while :; do echo a; fi; echo after` is
`'while' is missing its closing 'done'` (rc 1, POSIX rc 2). Write the closer
that matches the block; never put a foreign closing keyword in a body.

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
`>%var` and `&>%var` forms of the section above. The attached forms follow
the same rule on the **operand's own first byte**: a quoted `%` is ordinary
file-name data (`> "%v"`, `>| "%v"`, `>> "%v"`, `&> "%v"` open the file
`%v`, bash parity, task #329), only the unquoted `%` is the fd lookup, and
the `>&%var` dup word is the mask-blind arm above.

The operand word's **brace expansion** follows POSIX XCU 2.7 `worddef` and is
bash parity (task #324): an operand expanding to a number of **non-empty**
words other than one is `ambiguous redirect` (rc 1), the command runs nothing
and opens nothing. fdshell brace-expands before the parse, and the guard runs
on the operand region of the pre-parse text, so the whole command is refused
before any target is opened — `>{a,b} 2>z` creates no `z`, as bash, and a
redirect that **precedes** the ambiguous one is left unopened too:
`echo x >out >{a,b}` creates no `out`, where bash opens it before failing on
`{a,b}` (rc 1 in both shells). The matrix is `>{a,b}`, `> {a,b}`, `>>{a,b}`,
`<{a,b}`, `<> {a,b}`, `>|{a,b}`, `2>{a,b}`, `exec 3<{a,b}`, `>{a,b}x`,
`>x{a,b}`, `>{a,b}c{d,e}`, `>{1..3}`, `>{a,b*}`, `&>{a,b}`, `&>>{a,b}`,
`>%{a,b}`, `>&%{a,b}`, `<&{3,4}`, `<&{3..5}` and the zero-word `>{,}` /
`>{,,}`; `>{a}`, `>x{a}y`, `>{a..a}`, `>'{a,b}'` and `>{ 1 , 2 }` keep their
one word. Four measured residuals, each pinned: the *attached*
one-non-empty-word form `>{,zz}` is fdshell's `invalid redirect syntax`, where
bash counts the non-empty word and opens `zz` (fdshell's textual rewrite yields
`> >zz`), and braces in an io-number **prefix** produce several redirect words
(`cat {3,4}>f` runs one `cat` with fds 3 and 4 on `f`, rc 0, bash runs `cat 3`
and `cat 4`, rc 1), the word rule above. A **quoted** io-number prefix escapes
the rule on a path arm — an rc divergence: the operand offset is a
quote-stripped token offset, so the region starts at the prefix's **closing**
quote, the dangling `"` opens a quote, the brace counts as quoted and the
region yields one word, so `cat "2">{a,b}` and `cat '2'>{a,b}` keep the
last-wins rewrite (rc 0, **both** `a` and `b` created) where bash is rc 1 and
opens nothing; on a dup arm the closing quote falls *before* the region, so
`echo x "2">&{a,b}` does bail, rc 1 as bash, and the message names the operand
together with its operator byte (`>&{a,b}`, bash `{a,b}`). The async form
`echo x >{a,b} &` is refused in the parent (rc 1) where bash prints the
diagnostic in the child and its own status is 0 — the task #179 family.

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

The descriptor these operators open is **read-only**, as bash's
here-document/here-string source is: a `N>file`-style write to it is refused
`EBADF` and the body survives, so `exec 3<<<x; echo hi >&3; cat <&3` prints `x`
and reports `echo: write error: Bad file descriptor` (rc 0, and the failed
command's own status is 1). fdshell's report has no `bash: line 1: ` prefix —
that prefix is bash's, on every shell-reported write error. The machinery stays
divergent where it shows through: bash 5.3.9 backs the body with a **pipe**
opened read-only (so it is not seekable), fdshell with a memfd re-opened
`O_RDONLY` through `/proc/self/fd/N` (offset 0, seekable). Owning task: **#459**
(`> /dev/fd/N` onto such a descriptor: task #337 made fdshell **open** the magic
path like bash, so the shapes separate by what the fd is. On a file-backed fd
they agree — `exec 3< f; echo hi > /dev/fd/3` leaves `f` = `hi\n` in both (the
pre-#337 dup refused the write). On a here-body fd they do not: bash's is a
**FIFO**, so the `O_TRUNC` open finds a pipe, ignores the truncation and appends
— `exec 3<<<x; echo hi > /dev/fd/3; echo rc=$?; cat <&3` prints `rc=0`, `x`,
`hi` — while fdshell's memfd path re-opens as a fresh descriptor of the memfd
and truncates the body, printing `rc=0` and `hi` (measured 2026-10-28).

The `<<<` form's remaining differences from bash:

- `cat 3<<<<<x` is the here-string `<<x` on fd 3 (the pinned `cat <<<<<x`
  shape, fd-ified); bash rejects it as a syntax error (rc 2).
- A bare `3<<<` at end of input takes the empty word, printing a lone newline
  to fd 3; bash rejects it as a syntax error (rc 2).
- An expansion failure in the word is an error (`cat 3<<<$99999999999999999999`
  reports `index or value too large`); bash expands it to the empty word and
  prints a newline.
- A redirect written after a group's `}` is not attached to the group
  (`{ cat; } 3<<<x` is a parse error); owning tasks: #187/#191.

## A `/dev/fd/N` target is opened, not duplicated

`> /dev/fd/3` (and `/proc/self/fd/3`) opens the file descriptor 3 points at,
with the operator's flags, **as the redirection runs** — so it sees the fd state
the command's earlier redirections left, a pipeline stage's pipe ends included,
and the open is a fresh one: `>` truncates, `<` starts at byte 0. The **success**
of that open is the exception, third bullet below. bash behaves the same way and
fdshell matches it byte-for-byte on the direct spellings
(`tests/redirect.rs::dev_fd_target_is_a_fresh_open_of_the_file_that_fd_points_at`
and its neighbours, task #337). Three measured divergences remain, all
2026-10-28 against bash 5.3.9 with stdout and stderr piped:

- A target that only **expands** to the magic form keeps the consumer's
  pre-command open, because the magic reading is on the word as the parser wrote
  it: `f(){ echo hi; echo e >&2; }; p=/dev/fd/1; f > z 2> $p` leaves `z` =
  `hi\n` and prints `e` on stdout, where bash leaves `z` = `e\n\n` and prints
  nothing. The quoted, unexpanded `2> "/dev/fd/1"` matches bash. Owning task:
  **#479**.
- A target naming an fd that an **earlier def of the same command** opens fails
  here, because the consumer's pre-open pass opens every path word **before** any
  redirection is applied; bash applies them one by one, so its open finds the fd
  (rc 1 for fdshell, `redirect file open failed`, and a `>` target created
  empty). Measured with fd 3 closed in the invoking shell, bash rc 0 in all
  three: `echo e 3>&1 2> /dev/fd/3` prints `e`; `echo e 3> z 2> /dev/fd/3` prints
  `e` and leaves `z` empty, because fd 2 is `z`; `f 3>&1 > z 2> /dev/fd/3`
  (`f(){ echo hi; echo e >&2; }`) leaves `z` = `hi\n` and prints `e`. Where the
  fd **exists** the content rule above holds and both shells agree. Owning task:
  **#482**.
- `set -C` refuses a magic target whose descriptor is a **pipe**
  (`set -C; echo e > /dev/fd/1` → rc 1, `noclobber is set`); bash's open
  succeeds — Linux ignores `O_EXCL` for a non-regular file — and prints `e`. On
  a regular-file target both shells refuse, which is the pinned shape. Owning
  task: **#480**.

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

The one accepted divergence is the family of five keywords fdshell implements
with **no builtin of that name**: bash runs `'break' 1`, `'continue' 1`,
`'return' 3`, `'umask' 022` and `'unset' zz` as its **builtins** (the quoted word
is looked up, the builtin answers), where fdshell keeps the keyword. Those five
words stay quoted-blind **permanently**, and the quoted spelling is fully
transparent — it reaches the same keyword as the unquoted row and answers with
the same report at the same rc. Measured against bash 5.3.9 and pinned in
`safe/fdshell/tests/quotes.rs`: `'break' 1`, `"break" 1`, `b"reak" 1`, `''break
1` all report `'break' is not inside a loop` at rc 1, byte for byte the `break 1`
row (bash warns and exits 0 — that rc and message are the keyword's own accepted
outside-loop semantics, the family
`safe/fdshell/tests/loops.rs::break_2_outside_loop_aborts_rc1` pins, not a
quoting effect);
`for i in 1 2; do 'break'; echo after; done` prints **nothing** at rc 0 *because*
the quoted word is the keyword; `'umask' 022` sets the mask, `'unset' zz` removes
the variable, `'return' 3` returns from a function. The word's **text** is the
keyword, so a spelling whose text is not the name is a plain command word in
both shells (`br"ek"` is `brek`).

A builtin cannot take this work over, and that is why the sites stay ungated. A
command **argument** is an expansion surface, so a builtin's argv receives an
`%name` operand as the fd's *number* (with `%f` bound, `echo %f` prints `8`;
see "`%` is a file descriptor namespace") — `unset %name`, a documented feature,
would close nothing, while the keyword reads the raw token word and that is
precisely why it is a keyword. `break`/`continue`/`return` are in-process control
signals (`safe/fdshell/src/loop_control.rs`), and a pipeline stage runs in a
forked child, so a builtin raising one ends the child silently and controls
nothing. A quoted-only gate would leave **two implementations of one rule**, so
the quoted and unquoted spellings of one command answer differently — `'break' 1`
rc 0 from the builtin, `break 1` rc 1 from the keyword — and quoting must not
change what a command means.

The cost of the decision is accepted and pinned
(`tests/quotes.rs::the_five_names_have_no_builtins_pinned`): the
`command`/`builtin` prefix cannot reach a parse-level shell command, so
`command umask` is `"umask" not found` and `builtin break` is `"break" is not a
shell builtin`, rc 1, where bash runs its builtin at rc 0 (owning task **#473**);
`umask -p` and a second operand are parse errors here where bash accepts them,
and the bare `umask` prints its four digits with no trailing newline (**#472**,
quote-independent); `echo a | 'break'` is rc 1 where bash warns at rc 0 (**#381**,
builtins intercepted in a pipeline stage).

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
The same rule decides a **nested** definition: a definition's own body **slice**
is judged when the statement is parsed, while a definition written *inside*
another body is only text, judged when that function runs — so
`f() { & }` and `f() { f() { }; }` define at rc 0, and only their calls report
the parse error (`function definition body is empty` for the second), where bash
rejects the definition itself with rc 2 and prints nothing.

## Quoted separators are word data

POSIX §2.9.1 recognizes a separator (`;`, `&&`, `||`, `|`, `&`, `(`, `)`) as
syntax, and §2.3 removes quotes only **after** tokenization: a word whose raw
span carries a quote byte is a **word**, so `echo a ";" b` prints `a ; b` and the
quoted `;` splits nothing. fdshell applies that at every separator site of the
parse — the statement split, a block's body slice, the terminator in front of a
clause keyword, the closing-keyword walk, the pipeline's `|`, the background `&`,
the `case` clause's `|` / `)` / `;;`, a `wait` arm's pattern head, a definer's
`)`, the scoped-assignment prefix bound, and the here-document's delimiter word.

The gate is the token's **raw span**, one owner:
`safe/fdshell/src/parse/word_quoted.rs::span_word` (text **and** span). Not the
per-byte mask — `"";` is an empty word token **plus** a real `;`, and a zero-byte
quoted region has no mask bit to say so — and not `keywords::quoted::quoted_word`,
whose scan-back for the word's start breaks on the separator byte itself, so it is
sound only for a keyword word, which ends in a letter (LESSONS: "A separator-shaped
word needs the span gate"). The span form is also the only gate the sites that own
no `line` can compute. **Escape spellings need no gate at all**: `\;`, `\|`, `\)`
keep the pair in the token's *text* (`parse/token/escape.rs`), so no separator text
exists there to gate, and both shells read them as the literal byte.

The two scanners stay in lockstep, each in its own representation: the byte-level
scans read the **raw** bytes, where a quoted word's text is `";"` / `"|"` and no
separator row can fire (`parse::redirect::clobber::pipe::pipe_token_span` is this
rule for the `|`), and the token-level place machine reads the same fact through
`keywords::position::separator_data` / `word_place`, which leave a separator-shaped
word the place an ordinary word leaves — an argument, and a `for` word list that
**stays** a list (`for i in ";" do …` is one iteration over `;`).

The pinned divergences are the owners' own, not this rule's: the bare path
redirect of a quoted separator word (`echo x > ";"` — bash writes the file `;` at
rc 0, fdshell answers `invalid redirect syntax`, task **#475**); a `)` behind an
**argument** is arm text here, where bash rejects the clause with rc 2 (task
**#419**'s family, e.g. `case x in a) echo a ";" b) ;; esac`); and the
command-not-found **status** of a quoted separator run as a command (`X=1 ";"`) is
fdshell's rc 1 where bash answers 127, the shell-wide convention of STYLE §4 — the
name on stderr is the fact.

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
