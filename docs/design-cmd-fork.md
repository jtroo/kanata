# Design: Conditional Action Based on Command Exit Code

**Issue:** [#2062](https://github.com/jtroo/kanata/issues/2062)
**Status:** Proposal / Discussion
**Author:** Micah Alpern

## Problem

Users want to branch between actions based on the exit code of an external
command. The motivating use case is application-aware key behavior: run a script
that checks window state, then execute one action or another depending on the
result.

Today's options are insufficient:

- `cmd` is fire-and-forget. Exit codes are not captured (`cmd.rs:39-60` logs
  success/failure but never reads `output.status`).
- `cmd-output-keys` reads stdout and injects keystrokes, but only supports
  simple keys and delays — not complex actions like `tap-hold`.
- TCP + `push-msg` + fake keys can achieve this, but the complexity is
  disproportionate to the task.

## Architecture Constraints

The core difficulty, correctly identified by the issue author, is the
**CustomAction / keyberon Action boundary**.

| Concern | Lives in | Knows about |
|---------|----------|-------------|
| `fork`, `switch`, `tap-hold` | keyberon `Action` enum | Key state, timing, layers |
| `cmd`, `cmd-output-keys` | kanata `CustomAction` enum | OS processes, stdout |

keyberon is a generic keyboard firmware library parameterized over a custom
action type `T`. It returns `CustomEvent::Press(&T)` and
`CustomEvent::Release(&T)` to the caller (kanata), which processes them in
`handle_keystate_changes()` (`mod.rs:1466-1984`). There is no return path from
a `CustomEvent` handler back into keyberon's action dispatch.

**Fork** decides synchronously inside `do_action()` by checking
`self.states` for active keys (`layout.rs:2397-2424`). It calls
`do_action()` recursively on the chosen branch — the sub-action goes through
the full press/release lifecycle automatically.

**Switch** similarly decides inside `do_action()`, evaluating a boolean VM
against active keys, history, layers, and devices (`layout.rs:2425-2460`).
Matching actions are pushed to a local `action_queue` and processed in the same
tick.

A `cmd-fork` needs command execution (kanata-side) to inform action selection
(keyberon-side). Two approaches bridge this gap, with different tradeoffs. A
third (Layout API injection) is included for completeness but collapses into
Approach A under scrutiny.

## Approach A: Virtual Key Delegation

### Concept

The parser internally allocates two hidden virtual keys for each `cmd-fork`
action, storing the user's sub-actions at those virtual key positions in the
layout. At runtime, kanata executes the command synchronously, checks the exit
code, and presses the appropriate virtual key. keyberon handles the rest — the
sub-action goes through the normal press/release lifecycle.

### User syntax

```
(cmd-fork <action-if-0> <action-if-nonzero> cmd args...)
```

Example:

```
(defalias
  app-aware (cmd-fork
    (tap-hold 200 200 a lctl)
    (tap-hold 200 200 b lalt)
    bash -c "pgrep -x Firefox > /dev/null"
  )
)
```

### Parser changes

In `parser/src/cfg/`:

1. Add `cmd_fork.rs` with `parse_cmd_fork()`.
2. For each `cmd-fork`, allocate two entries in `s.virtual_keys` with generated
   names (e.g., `__cmd-fork-0-true`, `__cmd-fork-0-false`).
3. Store the sub-actions at those virtual key positions via `parse_action()`.
4. Produce `CustomAction::CmdFork { cmd, vk_true: KCoord, vk_false: KCoord }`.

### Runtime changes

In `src/kanata/mod.rs`:

**On press (`CustomEvent::Press`):**

```rust
CustomAction::CmdFork { cmd, vk_true, vk_false } => {
    #[cfg(feature = "cmd")]
    {
        let exit_code = run_cmd_sync(cmd);  // new fn in cmd.rs
        let chosen = if exit_code == 0 { vk_true } else { vk_false };
        self.active_cmd_forks.insert(coord, *chosen);
        handle_fakekey_action(FakeKeyAction::Press, layout, chosen.x, chosen.y);
    }
}
```

**On release (`CustomEvent::Release`):**

```rust
CustomAction::CmdFork { .. } => {
    #[cfg(feature = "cmd")]
    if let Some(chosen) = self.active_cmd_forks.remove(&coord) {
        handle_fakekey_action(FakeKeyAction::Release, layout, chosen.x, chosen.y);
    }
}
```

**New state in `Kanata` struct:**

```rust
active_cmd_forks: HashMap<KCoord, KCoord>,
```

**New function in `cmd.rs`:**

```rust
fn run_cmd_sync(cmd_and_args: &[&str]) -> i32 {
    let mut cmd = std::process::Command::new(cmd_and_args[0]);
    for arg in &cmd_and_args[1..] {
        cmd.arg(arg);
    }
    match cmd.output() {
        Ok(output) => output.status.code().unwrap_or(1),
        Err(e) => {
            log::error!("cmd-fork: failed to run command: {e}");
            1  // treat launch failure as nonzero (false)
        }
    }
}
```

### Tradeoffs

**For:**

- No keyberon changes. Everything stays in kanata's CustomAction layer.
- Full action lifecycle. Sub-actions go through normal press/release via
  virtual keys, so `tap-hold`, `layer-while-held`, `multi`, etc. all work.
- Simple user syntax. Familiar fork-like pattern.
- Follows existing patterns. Virtual key delegation is already used by
  sequences, `fake-key-on-release`, `fake-key-on-idle`, etc.
- Small, self-contained change. Could ship quickly.

**Against:**

- **It's a workaround, not a solution.** Virtual keys are being used as action
  indirection — smuggling references across the CustomAction/Action boundary
  through a mechanism not designed for it. Hidden generated names, a tracking
  HashMap, press/release delegation — all symptoms of working around the
  architecture rather than solving at the right layer.
- **One-tick delay.** `layout.event(Press)` is processed on the next tick
  (~1 ms). Negligible compared to command execution latency, but it's there.
- **Consumes virtual key slots.** Two per `cmd-fork` action. The limit is
  `KEYS_IN_ROW` (768), shared with user-defined virtual keys.
- **Not composable.** Binary only. If users later want to combine command
  results with layer checks or key history, they can't — it's a standalone
  action with no connection to `switch`.

## Approach B: Extend `switch` with an External Condition

### Concept

Add a new switch condition type that evaluates to true based on the exit code
of a command. This composes with the existing `switch` boolean VM — users can
combine command results with key state, layer checks, timing, device history,
etc. using `and`/`or`/`not`.

### User syntax

```
(switch
  ((cmd-exit-0 bash -c "pgrep -x Firefox"))
    (tap-hold 200 200 a lctl) break
  ()
    (tap-hold 200 200 b lalt) break
)
```

Or combined with other conditions:

```
(switch
  ((and (cmd-exit-0 bash -c "pgrep -x Firefox") (layer base)))
    a break
  ((cmd-exit-0 bash -c "pgrep -x Terminal"))
    b break
  ()
    c break
)
```

### The Execution Timing Problem

This is the central design question for Approach B. Switch evaluation happens
inside `do_action()` during `layout.tick()`, which lives in keyberon. Commands
must execute somewhere. Three sub-options:

#### B1: Callback from keyberon

Add a trait or closure parameter to switch evaluation:

```rust
// In keyberon/src/switch.rs
pub fn actions<F: Fn(u16) -> bool>(
    &self,
    // ... existing params ...
    external_condition: F,
) -> SwitchActions<...>
```

The `evaluate_boolean()` function calls the callback when it encounters an
`ExternalCondition(id)` opcode. kanata provides the closure that runs the
command (or returns a cached result).

**For:** Lazy evaluation — only commands that are actually reached get executed.
Caching is straightforward (evaluate once per tick, cache by ID).

**Against:** keyberon now depends on a caller-provided callback during its core
evaluation loop. This is a meaningful change to keyberon's contract — it goes
from "pure evaluation of internal state" to "evaluation with external side
effects." The `Fn` trait bound propagates through `Switch::actions()`,
`evaluate_boolean()`, and into `do_action()`.

#### B2: Pre-execution with results passed in

kanata runs all registered commands before `layout.tick()` and passes results
as a `HashSet<u16>` or `&[bool]` alongside `active_keys`, `historical_keys`,
etc.

```rust
pub fn actions(
    &self,
    // ... existing params ...
    external_results: &[bool],
) -> SwitchActions<...>
```

**For:** keyberon stays pure — it receives data, not callbacks. Same pattern as
existing context parameters. The opcode is just a lookup into the results
array.

**Against:** kanata must run ALL registered commands every time ANY switch is
evaluated, or it needs to figure out which switches are reachable from the
current key state and only pre-execute those commands. The former is wasteful
(running `pgrep` on every keypress even when the cmd-fork key isn't pressed).
The latter is non-trivial — it requires walking the layout to find reachable
switch actions, which duplicates keyberon's own evaluation logic.

#### B3: Hybrid — lazy pre-execution

A middle ground: kanata passes a lazily-evaluated results structure. On first
access for a given condition ID, the command runs and the result is cached for
the remainder of the tick.

This is functionally equivalent to B1 (callback) but wrapped in a data
structure rather than a bare `Fn`. It has the same tradeoff: keyberon's
evaluation triggers external side effects, just indirectly.

### Implementation sketch (B2, simplest keyberon change)

**keyberon changes:**

1. Add `ExternalCondition(u16)` to the `OpCode` enum in `switch.rs`.
2. Add `external_results: &[bool]` parameter to `evaluate_boolean()` and
   `Switch::actions()` (and all callers in `do_action()`).
3. When evaluating `ExternalCondition(id)`, look up `external_results[id]`.

**Parser changes:**

1. Add `(cmd-exit-0 cmd args...)` as a switch condition in
   `parse_switch_case_bool()` (`switch.rs:50-325`).
2. Assign each unique command a numeric ID.
3. Store the command registry in `ParserState` for kanata to access at runtime.

**Runtime changes:**

1. Before `layout.tick()`, run all registered commands and build the results
   array.
2. Pass results through to `layout.tick()` → `do_action()` → switch evaluation.
3. This requires modifying the `Layout::tick()` signature to accept external
   results, or storing them in a field on `Layout`.

### Tradeoffs

**For:**

- **Architecturally honest.** Solves the problem where branching already lives.
  `switch` is kanata's general-purpose conditional engine with 7 condition
  types, boolean operators, and fallthrough. Command results are a natural 8th
  input source.
- **Composable.** Users can combine command results with layer, key-history,
  timing, and device conditions freely.
- **Multi-way branching for free.** Multiple cases with different commands,
  using switch's existing infrastructure.
- **Reusable.** A generic `ExternalCondition(u16)` opcode could serve future
  condition sources beyond commands (IPC, file existence, environment
  variables, etc.).

**Against:**

- **keyberon changes are unavoidable.** Whether callback (B1) or data (B2),
  the switch evaluation signature changes. This touches `switch.rs`,
  `layout.rs` (`do_action`, `tick`), and all callers. The "keyberon stays
  generic" argument is real but weakened — it's already not purely generic
  (device_history was added as a parameter to switch evaluation).
- **Pre-execution problem (B2).** Running all registered commands on every
  keypress is wasteful. Selective pre-execution requires knowing which switches
  are reachable, which is hard. The practical answer might be "just run them
  all and accept the cost" if users have few cmd conditions — but it's
  inelegant.
- **Callback problem (B1).** Adds a trait bound that propagates through
  keyberon's core evaluation path. Changes keyberon from pure state machine to
  one with external effects during evaluation.
- **Larger change surface.** More files, more tests, more review burden.
- **Verbose for the simple case.** The binary "if script succeeds do X else do
  Y" case requires switch/case/break boilerplate.

## Approach C: `Layout::queue_action()` API

Add a public method to keyberon's `Layout` that lets kanata inject an action
for execution. On closer examination, this collapses into Approach A: the
circular reference between `Action<'a, T>` and `CustomAction` (which *is* `T`)
means sub-actions can't be stored directly in `CustomAction`. Any indirection
scheme (index table, coordinate lookup) ends up being virtual key delegation
with a different API surface. Included here for completeness but not a distinct
approach.

## Shared Design Decisions

### Exit code convention

Following POSIX convention (`test(1)`, `grep(1)`, `diff(1)`):

- **Exit 0** = condition true / success → first action (A) or condition
  satisfied (B)
- **Exit nonzero** = condition false / failure → second action (A) or condition
  not satisfied (B)
- **Launch failure** (command not found, permission denied) = treated as nonzero

This aligns with how `if`, `&&`, and `||` work in every POSIX shell. Scripts
should use `exit 0` / `exit 1` and avoid codes above 125 (reserved by shells
for signal-killed processes, command-not-found, etc.).

The binary 0/nonzero split covers the common case. Multi-exit-code matching
(as suggested in the issue comments) is better served by having the script
handle the branching internally and returning 0/1, or by using multiple switch
conditions. It should not be initial scope — it fights POSIX convention where
exit codes > 1 mean "error," not "different valid result."

### Blocking behavior

Both approaches execute commands synchronously, blocking the state machine for
the duration. The same caveat that applies to `cmd-output-keys` applies here:
long-running commands freeze keyboard processing. This should be documented.

A future async variant with a timeout could be added but is out of scope.
Synchronous matches `cmd-output-keys` precedent and avoids the complexity of
async action resolution.

### Feature gate

Both approaches require `danger-enable-cmd yes` in `defcfg`, consistent with
all other `cmd*` actions. The parser validation in `parse_cmd()` already
enforces this and should be extended.

## Summary

| | Approach A: Virtual Key Delegation | Approach B: Switch Condition |
|---|---|---|
| **keyberon changes** | None | New `OpCode`, modified evaluation signature |
| **Composability** | Binary fork only | Full boolean VM, combine with any condition |
| **Action lifecycle** | Via VK press/release delegation | Native — actions dispatched by `do_action()` |
| **Implementation size** | Small (1 new CustomAction, 1 new parser file) | Medium-large (keyberon + parser + runtime) |
| **Architectural fit** | Workaround — uses VKs as indirection | Native — extends the conditional engine |
| **Pre-execution cost** | None — command runs only when key pressed | Must solve selective vs. blanket execution |
| **User syntax** | Concise for binary case | Verbose for binary, powerful for complex |
| **Future extensibility** | Limited to binary fork | ExternalCondition opcode is reusable |

## Open Questions for jtroo

1. **Which approach do you prefer?** A is smaller and ships faster but is
   architecturally a workaround. B is cleaner but touches keyberon and has the
   pre-execution/callback design question.

2. **If B: callback or data?** B1 (callback/closure) gives lazy evaluation but
   adds a trait bound to keyberon's core path. B2 (pre-executed results array)
   keeps keyberon pure but requires running commands speculatively.
   `device_history` was added as a data parameter to switch — is that the
   preferred pattern?

3. **Action argument order.** The proposed syntax puts actions before the
   command: `(cmd-fork action-true action-false cmd args...)`. This mirrors
   `fork` but differs from `cmd-output-keys` where the command comes first.
   Is there a preferred convention?

4. **Is there appetite for both?** A as a quick binary shorthand, B as the
   long-term composable solution? Or would you prefer one path only?
