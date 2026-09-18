# FD Shell — agent guidance

Read [`STYLE.md`] and [`LESSONS.md`] before changing code; add new lessons there.

## Project

The project name is **`fdshell`**. All `yask_*` tools and agent dispatches use
this name. The yask MCP server (`yask_*` tools) is how every agent talks to
yask's task board; the fdshell project in yask holds all tracked work.

## Workspace
`resolver = "2"`; three `#![no_std]` crates: `safe/fdshell/` (bin, `forbid(unsafe_code)`, shell logic), `safe/builtins/` (lib, `forbid(unsafe_code)`, builtins), `unsafe/sys/` (lib, unsafe, syscalls — the only crate with raw fds). Safe crates never call libc. Syscall wrappers return `Result<_, SyscallError>` via `cvt()`. Platform: Linux x86_64 only.

## Lints
Deny: `clippy::unwrap_used`, `expect_used`, `indexing_slicing`, `undocumented_unsafe_blocks`, `map_err_ignore`, `result_unit_err`, `unused_io_amount`. Allow: `dead_code`, `clippy::todo`.

## Commands
`cargo build`; `cargo fmt`; `cargo clippy -- -D warnings`; `nix build` (→ `result/bin/fdshell`); `nix flake check --build-all` (fmt + clippy + nextest). Version from `safe/fdshell/Cargo.toml`; `git add` nix files first. `package.nix` params: `doFmt`, `doClippy`, `doTests`, `doCoverage`.

Environment quirks:

- New untracked files are **silently excluded from nix builds** (`src =
  lib.cleanSource ./.` reads the git tree). After creating files, run
  `git add -N <files>` so `nix build` / `nix flake check` / `nix build
  .#coverage` include them.
- `.opencode/` and `opencode.json` are opencode tool state, tracked in git,
  not part of the build.

## Execution pipeline (`safe/fdshell/src/`)
`script.rs` = `run_script` (split on `;`/`\n`), `cond.rs` = `run_cond_list` (split `&&`/`||`), `run.rs` = `run_one` (parse + dispatch). `if`/`fi`: split on space mid-segment to catch keywords; unmatched `if` → `EINVAL`. Separators apply only outside quotes.

## Testing
`cargo nextest run --status-level fail --show-progress none`; integration tests in `unsafe/sys/tests/` and `safe/builtins/tests/`; unit tests in separate `<module>/tests.rs` files (inline `mod tests {}` forbidden — STYLE.md §2.8). **Never `cargo test`** — its shared harness breaks `fork()`-based tests (hangs, fd corruption, interference). Every test binary runs under a 128MB virtual-address cap (`.config/nextest.toml` wrapper script: `prlimit --as=134217728`); infinite allocation aborts that test cleanly instead of tripping the host OOM killer. Nix builds need `util-linux` (stdenv lacks `prlimit`) — see `package.nix`.

## Coverage
`nix build .#coverage` (after `git add`) → `result/index.html` + `result/coverage-report.txt`.

## FD types
Spec: [`STYLE.md`] §5. No raw fds outside `unsafe/sys`.

## Builtins
SHELLFD tags are per-builtin constants (`c"openat2"`, `c"dirfd"`). Always `O_CLOEXEC` (strip via `dup` if needed). No hardcoded constants: `libc::` in sys, re-exported in safe crates. `mkdirat` race accepted.

## Errors
Spec: [`STYLE.md`] §4. Clean, concise, actionable. Cross-crate: `.change_context()`; add a variant if none fits; preserve the error chain.

## Multi-agent workflow

Development dogfoods yask through the MCP interface. An **Orchestrator** runs
a loop: pick the next task with `yask_get_next_task(project)`, hand it to the
matching subagent by **project + task number** (`Task #<n> in project
fdshell`, nothing more), repeat until nothing is actionable. Role-specific
instructions live in
`.opencode/agent/{orchestrator,planner,epic-planner,investigator,executor,
reviewer,judge}.md`; this file documents only what every agent must agree on.

### Dispatch (state → agent)

| Task state                          | Handled by          | Ends with                         |
| ----------------------------------- | ------------------- | --------------------------------- |
| `Todo` / `Planning` (Epic)          | Epic Planner        | stays `Todo` (prereqs set on subtasks) |
| `Todo` / `Planning` (Investigation) | Investigator        | `Review` (epics created, `investigation.md` attached) |
| `Todo` / `Planning`                 | Planner             | `In progress` (plan attached)     |
| `In progress` (Investigation)       | Investigator (re-work) | `Review` (session summary attached) |
| `In progress`                       | Executor            | `Review` (session summary attached) |
| `Review` (no review yet)            | Reviewer            | stays `Review` (review attached)  |
| `Review` (review attached)          | Judge               | `Done` (commit) or `In progress` (verdict) |
| `Todo`/`Planning` blocked on unmet prereqs | (skip until prereqs advance) | — |
| `Blocked` / `Backlog`               | nobody — waiting on user / unscheduled | — |
| `Done` / `Archived`                 | nobody — finished    | —                                   |

The dispatch mechanics are the Orchestrator's job (see its role file).

### Epic workflow

Epics are containers, not work items; planning is about organizing subtasks,
not producing an implementation plan. The flow:

1. An Epic in `Todo` is dispatched to the **Epic Planner**.
2. It reviews the tree (`yask_get_project`), creates missing tasks, sets
   prerequisites between subtasks, and defines execution order.
3. It sets **all direct child tasks as prerequisites of the Epic itself**
   (`yask_set_prerequisites`). That is the gate: `get_next_task` skips the
   Epic until all subtasks are done.
4. Subtasks flow through the normal pipeline.
5. When all subtasks are `Done`, `get_next_task` returns the Epic again and
   the Orchestrator dispatches the Judge to move it to `Done`.

An Epic **never enters `In progress`** — it stays in `Todo` until all its
subtasks complete, then jumps to `Done`. If subtasks already exist and are
properly ordered, the planner may skip creating new tasks and just attach an
`epic-plan.md` summary; the key output is the prerequisite links.

### Investigation workflow

Investigation tasks produce **no code** — they produce **Epics**:

1. An Investigation task in `Todo` (or re-dispatched from `In progress`
   after a verdict) goes to the **Investigator**.
2. It researches the topic (repo docs/code plus internet sources), then
   creates **one or several Epics** in `Backlog` with self-contained
   descriptions. The epics get **no subtasks and no prerequisites** — that is
   the Epic Planner's job.
3. It attaches `investigation.md` to every created epic, attaches a
   `session-summary.md` to the task, and moves the task to `Review`.
4. The task flows through the normal review pipeline; the Judge moves it to
   `Done` **without a commit**.
5. The epics stay in `Backlog` until the **user** moves them to `Todo`, where
   the Epic Planner splits them using the epic's `investigation.md` as scope.

### Handoff contract

- The Orchestrator hands a subagent a task by **project + task number** —
  `Task #<n> in project fdshell` and nothing more.
- Each subagent resolves the task itself with `yask_get_task(project, number)`
  and reads only the attachments relevant to its role (`yask_last_attachment`,
  pulling older ones only when needed).
- Subagents never touch anything outside their own task except when creating
  a new task for out-of-scope work (below).

### Attachment conventions

Filenames are the contract; content is markdown unless noted:

| filename           | writer       | content                                      |
| ------------------ | ------------ | -------------------------------------------- |
| `plan.md`          | Planner      | implementation plan                          |
| `epic-plan.md`     | Epic Planner | task breakdown, prerequisites, execution order |
| `investigation.md` | Investigator | result of investigation, attached to created epics |
| `session-summary.md` | Executor   | what changed, verification results, deviations |
| `review.md`        | Reviewer     | plan→code verification, findings, verdict    |
| `verdict.md`       | Judge        | what must be fixed (re-work round)           |
| `unblock.md`       | any agent    | why a task is `Blocked` and what unblocks it |

Attach markdown by writing a temp file under `/tmp` and calling
`yask_add_attachment` with `file_path`, `content_type: text/markdown`, and
the canonical `filename`.

### Confirmation rule

Any move/archive/restore/delete that would change the state of multiple tasks
returns `requires_confirmation` plus the affected list and applies nothing.
The cascade is the domain's intended behavior (prerequisite pull-along), so
inspect the affected list (it must contain only the task plus its dragged
prerequisites), then re-issue with `confirm: true`.

### Blocking

Any agent may move its task to `Blocked` when it cannot proceed without
external input (ambiguous requirements, missing information, a human
decision). Attach `unblock.md` **before** moving: why blocked, a concrete
list of what is needed (questions for the user, decisions, inputs), and the
resume state. Never dispatch or advance a `Blocked` task; when nothing else
is actionable the Orchestrator reports blocked tasks so the user can answer.
Once the user moves the task out of `Blocked`, it re-enters the pipeline at
its resume state.

### Commits

Only the **Judge** commits, and only when moving a task to `Done`: stage and
commit exactly that task's files with a concise conventional message (match
recent `git log` style). **Exception:** Investigation tasks reach `Done`
**without a commit** — they produce no code, only epics and documents inside
yask.

### When a new task arises during implementation

If an agent (Planner, Executor, Reviewer) spots work outside the current
task's scope: create it immediately with `yask_create_task`, move it to
`Todo`, and — if the current task depends on it — record that with
`yask_set_prerequisites`. Do **not** implement it inside the current task.
Work in dependency order: finish prerequisite tasks first (through `Done`),
then return to the task that depended on them.