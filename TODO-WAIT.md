# `wait` — design

Event-case over fd vars. Renamed from `await`; absorbs the legacy `wait` builtin and supersedes the separate `poll`/`epoll` builtin idea (poll/epoll are the internal engine, not builtins).

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
- `accept %listenfd %array limit N` — see the `accept` arm below
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

## The `accept` arm

`accept %listenfd %array limit N) logic ;;`

- the mechanism accepts the connection and passes it to the arm as `%?`
- arm exits 0 → the parent appends the fd to `%array`
- arm exits non-0 → the fd is closed (rejected)
- `%array` already has N or more elements → accept + close immediately, the arm is not invoked (RST if unread data is buffered — acceptable for a cap)
- the append happens in the parent — arm children never mutate arrays, which is what makes this work across the fork boundary

## One-shot form (legacy `wait`)

- `wait %p1` (no arms) — blocking one-shot wait on a pidfd / pidvar, sets `$?`; disambiguated from the arm form by the arm's `)`
- a firing `finished %pidfd)` arm preloads `$?` with the child's exit status (harvested via `waitpid` at fire time) so arm bodies read codes bash-style
- `wait` moves from the intercept table (`intercept/mod.rs:24`) to the keyword layer (`parse/dispatch.rs`, like `while` / `case`); the `after N)` arm subsumes the `wait --any` + `--timeout` half of the `timerfd` TODO item
- background-task pidvar machinery untouched — `finished %pidvar)` on a background job just works

## Open issues

- `break` in an arm exits the child, not the parent's loop — propagate via the `wait` block's `$?` or an explicit sentinel
- backlog drain at the cap: one accept+close per poll round, or burst-drain while the array is full?

## Proof of concept — echo server

```
listen --bind=127.0.0.1 --port=8192 %>%listener
%conns=[]

while true; do
    wait
        accept %listener %conns limit 64)
            : ;;
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
