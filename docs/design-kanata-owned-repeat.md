# Design: Kanata-Owned Key Repeat

## Problem

On macOS, kanata intercepts physical keyboard input at the IOKit HID level and
emits output through the Karabiner virtual HID driver. The virtual HID sends
**entire keyboard reports** (a bitmap of all currently-held keys) rather than
individual key events like Linux's evdev. When kanata holds a key in the output
report longer than the OS repeat delay — even by a few milliseconds — macOS
triggers autorepeat that the user did not intend.

This causes real text corruption under load:

- `stillllllll`
- `aaaaaaaaaand`
- `forgottenn`

The root cause is that kanata's tap-hold processing delays key releases while
it resolves the tap vs hold decision. During that delay, macOS sees the
**previous** key as still held and begins repeating it.

### Why existing mitigations fail on macOS

| Mitigation | Works on Linux | Works on macOS |
|---|---|---|
| `(multi f24 (tap-hold ...))` interrupt trick | Yes | No — macOS does not reset its repeat timer on f24 |
| `linux-x11-repeat-delay-rate` | Yes (calls `xset`) | N/A — no macOS equivalent |
| `allow-hardware-repeat no` | Suppresses repeat events | Suppresses repeat events but provides **no replacement** — user loses all repeat |
| Increasing OS repeat delay | Partially | Partially — one tick longer works but degrades repeat UX for all keys |

### The macOS HID report model

On Linux, kanata sends individual `EV_KEY` events with `value=0` (release),
`value=1` (press), or `value=2` (repeat). The kernel and DE handle repeat
independently based on press/release timing. Linux's evdev is a higher-level
abstraction that converts raw HID reports into per-key events before userspace
sees them.

On macOS, the IOKit HID framework delivers raw HID reports directly to the
input subsystem. This is not a Karabiner design choice — it is a requirement
of the USB HID 1.11 specification and the IOKit framework. Every HID keyboard
device (USB, Bluetooth, or virtual) sends complete state snapshots: a modifier
bitmap plus an array of all currently-held keys. The HID Report Descriptor in
the Karabiner DriverKit driver defines this format:

```
Report Count: 32    (up to 32 simultaneous non-modifier keys)
Report Size:  16    (bits per key slot)
Input: (Data, Array, Absolute)
```

The Karabiner virtual HID driver maintains a `keyboard_input` struct
containing a `modifiers` bitfield and a `keys` set. Each call to
`async_post_report()` sends this entire state to macOS. macOS then infers
key activity from differences between consecutive reports.

This means:
1. There is no per-key "press" or "repeat" event at the HID level — only
   full keyboard state snapshots
2. Any processing delay that keeps a key in the report triggers OS autorepeat
3. The OS repeat delay setting is a **global** threshold that applies to all
   keys uniformly
4. The `f24` injection workaround fails on macOS because inserting `f24` into
   the report does not remove the original key — macOS still sees it as held

## Proposed Solution

Kanata takes ownership of key repeat generation:

1. **Suppress OS repeat**: set `allow-hardware-repeat no` implicitly when
   kanata-owned repeat is enabled
2. **On key press output**: start a per-key repeat timer
3. **After delay**: emit the first `KeyValue::Repeat` event
4. **At configured rate**: continue emitting repeat events
5. **On key release output**: cancel the timer

This gives kanata full control over when repeat events are generated,
eliminating the race between tap-hold processing delay and OS repeat threshold.

### Bonus: per-key repeat rates

Because kanata owns the repeat timers, different keys can have different
repeat timing:

- **Navigation keys** (arrows, pgup/pgdn): shorter delay, faster rate
- **Deletion keys** (backspace, delete): shorter delay, faster rate
- **Alpha keys**: standard or longer delay, standard rate
- **Modifier keys**: no repeat (already the case)

This is impossible with OS-level repeat, which applies one global setting.

## Configuration

### defcfg options

```
(defcfg
  ;; Enable kanata-owned repeat (default: disabled)
  ;; When enabled, OS repeat is automatically suppressed.
  managed-repeat yes

  ;; Default repeat timing (applies to all keys unless overridden)
  ;; delay = ms before first repeat, interval = ms between repeats
  managed-repeat-delay 600
  managed-repeat-interval 33  ;; ~30 repeats/sec
)
```

### defrepeat block (optional, per-key overrides)

```
(defrepeat
  ;; (key delay interval)
  ;; delay and interval in milliseconds
  (bspc 400 20)    ;; backspace: faster repeat
  (del  400 20)    ;; delete: faster repeat
  (left  300 25)   ;; arrows: short delay, fast rate
  (right 300 25)
  (up    300 25)
  (down  300 25)
  (pgup  400 30)
  (pgdn  400 30)
)
```

Keys not listed in `defrepeat` use the global defaults from `defcfg`.

Modifier keys (shift, ctrl, alt, meta) never repeat regardless of
configuration.

### Naming rationale

"managed-repeat" rather than "kanata-repeat" because:
- It describes the behavior (kanata manages repeat) not the implementation
- Consistent with kanata's naming style (e.g., `process-unmapped-keys`)
- Avoids implying this replaces the `rpt` action (which is a different feature)

## Implementation

### Data structures

```rust
// In parser/src/cfg/defcfg.rs

/// Per-key repeat timing override.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct ManagedRepeatEntry {
    pub key: OsCode,
    pub delay_ms: u16,
    pub interval_ms: u16,
}

// New fields in CfgOptions:
pub managed_repeat: bool,            // default: false
pub managed_repeat_delay: u16,       // default: 600
pub managed_repeat_interval: u16,    // default: 33
pub managed_repeat_keys: Vec<ManagedRepeatEntry>,  // from defrepeat block
```

```rust
// In src/kanata/mod.rs — new module: managed_repeat.rs

/// Tracks the repeat state for a single held key.
#[derive(Debug)]
struct RepeatTimer {
    /// The output key code being repeated.
    osc: OsCode,
    /// Ticks remaining before the first/next repeat fires.
    ticks_remaining: u16,
    /// The interval (in ms/ticks) between repeats after the first.
    interval: u16,
    /// Whether the initial delay has passed (first repeat has fired).
    repeating: bool,
}

/// Manages all active repeat timers.
#[derive(Debug, Default)]
pub struct ManagedRepeatState {
    /// Active timers, keyed by physical input OsCode.
    timers: HashMap<OsCode, RepeatTimer>,
    /// Per-key timing overrides from defrepeat config.
    overrides: HashMap<OsCode, (u16, u16)>,  // (delay, interval)
    /// Global defaults.
    default_delay: u16,
    default_interval: u16,
}
```

### New fields in Kanata struct

```rust
pub struct Kanata {
    // ... existing fields ...

    /// Kanata-owned repeat state. None when managed-repeat is disabled.
    pub managed_repeat_state: Option<ManagedRepeatState>,
}
```

### Integration points

#### 1. Key press detection — `tick_states()` in mod.rs

After `handle_keystate_changes` detects new key presses (comparing `cur_keys`
vs `prev_keys`), start repeat timers for newly pressed keys:

```rust
fn tick_states(&mut self, _tx: &Option<Sender<ServerMessage>>) -> Result<()> {
    self.live_reload_requested |= self.handle_keystate_changes(_tx)?;
    // NEW: after keystate changes are processed, update managed repeat timers
    self.tick_managed_repeat()?;
    self.handle_scrolling()?;
    // ... rest of tick_states ...
}
```

#### 2. Repeat tick processing — new `tick_managed_repeat()` method

Each tick (1ms):
1. For each active timer, decrement `ticks_remaining`
2. When a timer fires (`ticks_remaining == 0`):
   - Emit `write_key(&mut self.kbd_out, osc, KeyValue::Repeat)`
   - Reset `ticks_remaining` to `interval`
   - Set `repeating = true`
3. For keys in `prev_keys` but not in `cur_keys` (just released):
   - Remove the timer
4. For keys in `cur_keys` but not in `prev_keys` (just pressed):
   - If not a modifier, create a timer with the appropriate delay

#### 3. OS repeat suppression

When `managed_repeat` is enabled:
- `allow_hardware_repeat` is forced to `false`
- On macOS, the existing `KeyValue::Repeat` filter at line 185 of
  `src/kanata/macos.rs` already handles this
- On Linux, the existing filter handles this too

This means OS repeat events are dropped, and only kanata-generated repeats
reach the output.

#### 4. Config reload

On live reload, `ManagedRepeatState` is reconstructed from the new config.
Active timers are cleared (keys that are physically held will get new timers
on the next tick when they appear in `cur_keys`).

### Output path on macOS (validated)

When kanata emits `KeyValue::Repeat` via `write_key`, the macOS backend
(`KbdOut::write_key` in `oskbd/macos.rs`) calls `async_post_report` which
sends the full keyboard report to the Karabiner DriverKit driver.

In the driver's `send_key()` (`driverkit.cpp`), for a key already held:
```cpp
keyboard.keys.insert(e->code);   // no-op: key already in set
client->async_post_report(keyboard);  // sends identical report
```

The `insert()` call is a no-op (the key is already in the set), but
`async_post_report` is still called, posting the same report again. macOS
treats each posted report as a new input signal, producing a character even
though the report content is identical to the previous one.

**This was validated on real macOS hardware (2026-05-14).** Managed repeat
at 30ms interval produced visible character repetition in a text editor,
with `managed repeat A` log lines confirming kanata-generated repeats.

Release+re-press is NOT required. The simpler approach of re-posting the
same HID report works because macOS processes each `async_post_report` call
as a discrete input event.

## Scope and Phasing

### Phase 1: Prototype — COMPLETE

- Added `managed-repeat`, `managed-repeat-delay`, `managed-repeat-interval`
  to defcfg parsing
- Implemented `ManagedRepeatState` and `tick_managed_repeat()` in new
  `src/kanata/managed_repeat.rs` module
- Wired into `tick_states()` and `is_idle()` (prevents processing loop from
  blocking when repeat timers are active)
- Forces `allow-hardware-repeat no` when managed-repeat is enabled
- 6 simulation tests passing (basic, early release, modifier exempt,
  disabled-by-default, delay boundary, layer-while-held)
- Validated on real macOS hardware: repeat produces visible characters
  through the Karabiner virtual HID without release+re-press

### Phase 2: Per-key overrides (current)

- Add `defrepeat` block parsing as a new top-level config form
- Populate `ManagedRepeatState.overrides` from parsed config
- Config validation (warn on modifier keys in defrepeat)
- Additional sim tests for per-key timing
- Documentation in config.adoc

### Phase 3: Polish and testing

- Test under CPU load (original MAL-57 scenario)
- Verify tap-hold interactions
- Downgrade `managed repeat` log line to `log::debug!`
- Update design doc with load test results

### Phase 4: Upstream proposal

- Open discussion on jtroo/kanata with:
  - Problem statement (macOS HID report model + DriverKit source analysis)
  - Reproduction evidence from KeyPath investigation
  - Working prototype with test results
  - Config syntax proposal
- Frame as macOS-focused but cross-platform capable
- Reference Issue #1441 and Discussion #422

## Risks and Mitigations

| Risk | Mitigation |
|---|---|
| Repeat feels different from OS repeat | Match default delay/interval to macOS defaults (600ms / 33ms) |
| Breaks modifier holding (games, shortcuts) | Never repeat modifier keys; only repeat when output is a non-modifier keycode |
| Performance: HashMap lookup per tick per held key | Bounded by ~6 simultaneously held non-modifier keys in practice; negligible |
| Conflicts with `rpt` action | Orthogonal — `rpt` repeats last action on a different key; managed-repeat repeats the held key itself |
| jtroo rejects upstream | Feature is opt-in; macOS case is strong; worst case stays in KeyPath fork |

## Testing Strategy

### Simulation tests

```
;; Basic managed repeat test
(defcfg managed-repeat yes managed-repeat-delay 5 managed-repeat-interval 3)
(defsrc a)
(deflayer base b)

;; Press a, wait 5ms → first repeat of b
;; Wait 3ms → second repeat of b
;; Release a → no more repeats
```

### Manual macOS test

1. Enable managed-repeat with short delay (200ms) and fast interval (20ms)
2. Hold a key in a text editor
3. Verify repeat starts after ~200ms
4. Verify repeat rate matches ~50/sec
5. Verify no spurious repeats with tap-hold under load
6. Compare feel to OS repeat

## References

- [KeyPath MAL-57: Duplicate Key Presses Under Load](https://github.com/user/keypath/docs/bugs/MAL-57-duplicate-keypresses.md)
- [KeyPath investigation: Duplicate Key Under Load](https://github.com/user/keypath/docs/analysis/2026-03-07-duplicate-key-under-load-investigation.md)
- [kanata Discussion #422: Layer keys sometimes lead to duplicate keypresses](https://github.com/jtroo/kanata/discussions/422)
- [kanata Issue #1441: double-pressing a tap-held key leads to 3-4+ letters](https://github.com/jtroo/kanata/issues/1441)
- [kanata Issue #2042: Windows stopped sending repeat events](https://github.com/jtroo/kanata/issues/2042)
- [kanata Issue #1794: Key release delayed by tap-hold timeout](https://github.com/jtroo/kanata/issues/1794)
