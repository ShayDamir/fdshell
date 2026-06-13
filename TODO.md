# TODO

## Refactoring / cleanup

- [ ] File length: `run.rs` (81 code lines, 1 over), `if_block.rs` (95 code lines, 15 over), `script.rs` (81 code lines, 1 over) — minor extractions to get under 80
- [x] Add tests for `run.rs` else/elif execution paths (`if false; then ...; else ...; fi`, elif-first-fails, all-elifs-fail-no-else)
- [ ] Add `exec_fd`/`exec_at` to `safe/builtins/` crate (parse modules + integration tests)
- [ ] Drop `no_std` on `unsafe/sys` — replace `IoVec`/`IoVecMut` with `std::io::IoSlice`/`IoSliceMut`

## Bash compatibility gaps

### P0 — Easy wins

- [x] `$?`
- [x] `true` / `false` builtins — exit 0 / exit 1 without PATH lookup
- [x] `pwd` — print CWD path via `std::env::current_dir()`
- [x] Tilde expansion (`~`, `~user` literal) → `$HOME`
- [x] `$$` — shell PID in dollar_subst
- [ ] `${#var}` — string length expansion
- [x] `$!` — last background PID
- [ ] `$_` — last argument of previous command
- [ ] `$-` — shell option flags
- [ ] `help` builtin — list available builtins
- [ ] `type` builtin — show command type (builtin, external, fd var, etc.)
- [ ] `command` builtin — bypass function lookup (alias for `builtin` prefix)

### P1 — Major functionality gaps (moderate effort)

- [ ] `test` / `[` builtin — file tests (`-f`, `-d`, `-e`), string tests (`=`, `!=`, `-z`, `-n`), numeric tests (`-eq`, `-lt`, `-gt`)
- [x] `while` / `until` loops — shared LoopBlock struct + parser in while_block.rs, separate runner arms with invert flag
- [x] `export` — shell state exports map, env vars passed to children via exec_fd/exec_at; builtin supports `export VAR=val`, `export VAR`, and `export` (list)
- [ ] `read` builtin — read line from stdin, split into variables
- [ ] `printf` builtin — format string output
- [ ] `set --` + positional params — `$1`..`$9`, `$#`, `$@`, `$*`, `$0`
- [ ] `${var:-default}`, `${var:=default}`, `${var:+alt}`, `${var:?err}` — parameter expansion operators
- [ ] `${!indirect}` — indirect variable expansion
- [ ] `exec` builtin — rename/adapt `become`; also support redirect-only mode `exec N>&file`
- [ ] `eval` — parse and execute constructed string
- [ ] `source` / `.` — execute script file in current shell
- [ ] `shift` — shift positional parameters
- [ ] `break` / `continue` — loop control
- [ ] Here-strings (`<<<"string"`) — pipe string into command's stdin
- [ ] `>&` / `<&` fd dup redirects — `echo hello 2>&1`, `exec 5>&1`
- [ ] `&>file` — combined stdout+stderr redirect
- [ ] `<>` — open file for read/write
- [ ] `/dev/fd/N` — automatic fd path translation
- [ ] Word splitting after unquoted `$` expansion — split on IFS when assignment is unquoted
- [ ] `shopt` / `set -o` — shell options bitmask
- [ ] Alias expansion — text-replacement pass on command words
- [ ] `hash` — PATH lookup cache
- [ ] `ulimit` — resource limit get/set
- [ ] `return` builtin (requires functions)

### P2 — Important for bash compatibility (hard)

- [ ] Heredocs (`<<EOF`) — multi-line stdin from script body with delimiter scanning
- [ ] Glob expansion (`*`, `?`, `[...]`) — expand patterns to matching filenames
- [ ] Arithmetic expansion `$((expr))` — integer expression evaluation
- [ ] Brace expansion (`{a,b,c}`, `{1..5}`) — pre-tokenization string generation
- [ ] Functions — `name() { body; }` with scoped locals, call stack, `return`
- [ ] `case` / `esac` — pattern matching with `;;` separators
- [ ] Job control — `bg`, `fg`, `jobs`, SIGTSTP handling, TTY pgrp management
- [ ] Process substitution (`<(cmd)`, `>(cmd)`) — fifo/pipe with /dev/fd path
- [ ] `"$@"` preservation — expand to multiple words preserving empty args
- [ ] History expansion (`!!`, `!echo`) — readline-style history
