# `wait` — design

Event-case over fd vars. Renamed from `await`; absorbs the legacy `wait` builtin and supersedes the separate `poll`/`epoll` builtin idea (poll/epoll are the internal engine, not builtins). Arms return fds to the parent via bounded capture (`%>%arr[N]` / `%tag>%arr[N]`) — a general capture feature (TODO.md) that the `readable %listener` accept idiom builds on.

## Syntax

```
wait
    PATTERN) arm commands ;;
    PATTERN) arm commands ;;
done
```

case/esac grammar, terminated by `done`. Patterns:

- `readable %fd` / `writable %fd` — poll/epoll readiness on a single fd var
- `finished %fd` — uniform end-of-life: stream EOF/hangup for sockets, exit for pidfds — one arm covers network peers and background tasks alike
- Bounded capture `%>%arr[N]` / `%tag>%arr[N]` — arm sends append to `%arr` up to N elements; `%>` is untagged (matches any tag), `%tag` captures only on a matching tag (general capture feature usable by any command — TODO.md; see Bounded capture below)
- `after N` — Erlang `receive … after`, for heartbeats
- `%arr[]` wildcard — any element of an fd-var array

The matching fd is bound to `%?` (fd namespace — no clash with `$?` exit status).

## Core semantics

- One-shot: each `wait` block is one poll round. A forked arm child per ready fd; the block returns once the round is dispatched. A server wraps it: `while true; do wait … done`.
- Arms run in child processes — no green threads / suspension points in a shell. Children may block freely (a partial line read blocks only its own conn).
- Reentrancy guard: fds with a live arm child are excluded from the poll set until that child exits (kernel buffers the meantime data). The children's pidfds are in the same poll set, so reaping is non-blocking and the parent never stalls.
- The parent owns the polled fds throughout; the arm child's dup is ephemeral.

## Keep / release protocol — the arm's exit status

- exit 0 → the fd stays open, pollable next round
- non-0 → the parent unsets + closes it (all copies gone → FIN reaches the peer — no `unset` needed inside the arm)
- `finished` arms close unconditionally (an EOF'd socket / reaped pidfd has no future); the exit status of a reaped pid stays queryable via the one-shot `wait %p1`, bash-style
- Arm bodies must end with an *intentional* status (an incidental short read must not be conflated with release)

## Bounded capture — `%>%arr[N]` / `%tag>%arr[N]`

Bounded array capture is a general extension of fd capture (TODO.md) — any command can declare it, not just `wait` arms; decomposing the array is the for-loop-over-arrays item. Arms use it to send fds back to the parent with `send_fd` (tagged) before they exit — zero, one, or many. The declared form selects which sends are captured: `%>` is untagged (any tag), `%tag` only a matching tag. The parent drains the arm's capture socket after the arm's pidfd fires: non-blocking, so it exits once the kernel buffer is empty — the arm is dead, it cannot produce more, and grandchildren holding the socket end cannot stall it. The drain also ends at the first message from a sender pid other than the arm (reject; the arm's own sends were already buffered). Each received fd appends to the declared array until it holds N elements; further fds are closed (RST if unread data is buffered — acceptable for a cap). Zero received is legal. The mechanism is general — any arm can return fds this way, not just accept.

## Connections — the `readable %listener` arm

No dedicated `accept` pattern: a readable listener means a pending connection, so the arm runs the `accept` builtin itself and returns the result via bounded capture. The matched fd (`%?`) is the listener.

```
readable %listener %accept>%conns[64])
    builtin accept %listener %>%new_conn || exit 0
    builtin send_fd accept %new_conn ;;
```

- exit status governs the *listener* (the matched fd), per the keep / release protocol above: non-0 → parent unsets + closes it. Transient accept failures (ECONNABORTED — routine, and what port scanners deliberately send) must exit 0 (`|| exit 0`) to keep the listener open; the hardened idiom above is *the* documented form — the naive two-line body dies on the first RST probe
- backlog drain: while the listener is still readable the arm can accept in a loop and `send_fd` each conn — the parent appends them all until the array is full; drain speed is one arm per round, rounds back-to-back

## One-shot form (legacy `wait`)

- `wait %p1` (no arms) — blocking one-shot wait on a pidfd / pidvar, sets `$?`; disambiguated from the arm form by the arm's `)`
- a firing `finished %pidfd)` arm preloads `$?` with the child's exit status (harvested via `waitpid` at fire time) so arm bodies read codes bash-style
- `wait` moves from the intercept table (`intercept/mod.rs:24`) to the keyword layer (`parse/dispatch.rs`, like `while` / `case`); the `after N)` arm subsumes the `wait --any` + `--timeout` half of the `timerfd` TODO item
- background-task pidvar machinery untouched — `finished %pidvar)` on a background job just works

## Signals

No `signal)` pattern. The shell installs no handlers (there is no `trap` today), so while the parent blocks in the poll wait, default-disposition signals act exactly as at any other blocking point: SIGINT / SIGTERM terminate the shell, and group-directed signals (Ctrl+C) also reach the live arm children (same pgrp) — arms never outlive the wait. Custom handling arrives with the `signalfd` direction (TODO.md): a trapped signal is an fd source, so the signalfd is just another polled fd (`readable %sigs)` in the same block). Implementation note: the poll wait must loop on `EINTR` — a handler installed later will interrupt it; signalfd itself never does (the kernel routes the signal to the fd instead).

## Open issues

- `break` in an arm exits the child, not the parent's loop — propagate via the `wait` block's `$?` or an explicit sentinel

## Proof of concept — echo server

```
listen --bind=127.0.0.1 --port=8192 %>%listener
%conns=[]

while true; do
    wait
        readable %listener %accept>%conns[64])
            builtin accept %listener %>%new_conn || exit 0
            builtin send_fd accept %new_conn ;;
        readable %conns[])
            read -u %? REQUEST
            if [ "$REQUEST" = "done" ]; then
                exit 1
            else
                echo "pong" >%?
            fi ;;
        finished %conns[]) : ;;
    done
done
```

~20 lines — validates the P3 application domains: supervisors / watchers / services as scripts.
