# Technical Plan: Issue #1097 — Physical Key Event Visibility

## Status and scope

GitHub issue [#1097](https://github.com/jtroo/kanata/issues/1097), “Feature request: Show keystroke or can let other app know the real key is stroked,” is open. The requested behavior has not been implemented in full.

The current tree has two related facilities:

- `push-msg` can emit user-authored TCP notifications, but requires every interesting action to be annotated in the configuration.
- `HoldActivated` and `TapActivated` automatically report the physical key for tap-hold resolution, but do not report ordinary physical key presses and releases.

This plan adds a structured TCP notification for the original physical input that Kanata accepts from `defsrc`, before remapping. It does not add a built-in graphical keystroke overlay; visualization remains the responsibility of a TCP client. This follows the issue author's stated alternative and reuses Kanata's existing cross-platform notification path.

## Goal

Allow an external key-visualization or accessibility application to learn which configured physical key the user pressed or released, even when Kanata suppresses or replaces that key's output.

## Proposed acceptance criteria

- With the TCP server enabled, a client can receive a structured event for each physical `defsrc` key state change that reaches Kanata's processing loop.
- The event identifies the original physical key independently of the remapped output.
- Press and release are distinguishable. The treatment of hardware repeat and `KeyValue::Tap` is resolved by Open Decision 1.
- Events are emitted from the shared processing path and therefore behave consistently across Linux, macOS, Windows low-level hook, and Windows Interception input backends.
- Synthetic/fake-key actions and Kanata's generated output do not masquerade as physical input.
- Existing TCP clients and existing server messages continue to work unchanged.
- A client can use the `Hello` response to detect support before relying on the event.
- The TCP protocol documentation and example client show the exact JSON shape and lifecycle.
- Unit tests cover serialization, physical-event conversion/filtering, and emission without requiring real keyboard hardware.

## Repository findings

### Input and notification flow

- Platform event loops convert OS events to `KeyEvent` and send them to the shared processing loop: `src/kanata/linux.rs`, `src/kanata/macos.rs`, `src/kanata/windows/llhook.rs`, and `src/kanata/windows/interception.rs`.
- Unmapped keys are passed through by the platform loops instead of being sent to the shared processing loop. Consequently, the events consumed by `Kanata::start_processing_loop` in `src/kanata/mod.rs` are the appropriate cross-platform approximation of “physical keys in `defsrc`.”
- `Kanata::start_processing_loop` batches/sorts received events and calls `handle_input_event` before advancing the layout in `src/kanata/mod.rs`. This is the single shared point where the original `KeyEvent` is still available and has not been converted into a remapped output.
- The processing loop already owns an optional `SyncSender<ServerMessage>`. `Kanata::start_notification_loop` receives those messages and broadcasts them to connected TCP clients.
- The wire model and serialization tests live in `tcp_protocol/src/lib.rs`; the TCP command/capability implementation lives in `src/tcp_server.rs`.
- User-facing TCP protocol documentation is in `docs/config.adoc`, and a consumer implementation is in `example_tcp_client/src/main.rs`.

### Existing naming and protocol patterns

- `OsCode` implements `Display` in `parser/src/keys/mod.rs`. Existing `HoldActivated`/`TapActivated` code converts it with `osc.to_string().to_lowercase()`, providing a precedent for physical-key names.
- `ServerMessage` uses externally tagged serde enums and newline-delimited JSON. Adding a new enum variant is additive for the wire format, though clients that exhaustively deserialize/match the Rust enum must update.
- Optional TCP functionality is consistently guarded by `#[cfg(feature = "tcp_server")]`.
- Feature discovery uses `ClientMessage::Hello` and `ServerMessage::HelloOk { capabilities, ... }`.
- Notification-channel backpressure is handled with non-blocking `try_send` and logging in the processing path; socket write failure removes stale clients in the notification loop.

## Design

### 1. Define the wire event

In `tcp_protocol/src/lib.rs`, add a `ServerMessage` variant for original physical input. Its final name and fields depend on Open Decision 1. At minimum, it must carry:

- a stable physical key name derived from `OsCode`, consistent with `HoldActivated` and `TapActivated`; and
- a state that lets a visualization client distinguish press from release.

Do not place remapped output keys in this message. The event describes the input-side `KeyEvent` only.

Add the corresponding capability string to the `HelloOk` response assembled in `src/tcp_server.rs`. Keep the existing protocol number unless maintainers explicitly require a bump for additive server events; the established capability mechanism is sufficient for negotiation.

### 2. Convert input events in one shared helper

Add a small `#[cfg(feature = "tcp_server")]` helper in `src/kanata/mod.rs` near the processing-loop code that converts `&KeyEvent` into `Option<ServerMessage>`.

The helper should:

- use `event.code.to_string().to_lowercase()` for the human-readable key name, matching current physical-key notifications;
- retain the numeric OS code only if Open Decision 1 selects it as part of the contract;
- map or filter `KeyValue` variants according to Open Decision 1;
- filter internal wake-up/sentinel events such as `KeyValue::WakeUp` / `KEY_RESERVED` so TCP commands and reload wakeups are never reported as keystrokes; and
- remain a pure conversion where practical, so all edge cases can be unit-tested without sockets or platform input devices.

Keeping this conversion outside individual OS backends prevents behavior drift and avoids duplicating notification code across four input implementations.

### 3. Emit before remapping

In both receive branches of `Kanata::start_processing_loop` in `src/kanata/mod.rs` (blocking `recv` and non-blocking `try_recv`), emit converted physical-input notifications for the collected/sorted batch immediately before each event is passed to `handle_input_event`.

Use a shared batch-processing helper or a narrowly scoped emission helper so the two branches cannot diverge. Use the existing optional TCP sender and `try_send` convention; a slow or absent visualization client must not block the keyboard processing loop. Log a dropped notification at an appropriate rate/level without failing keyboard processing.

This location deliberately means:

- physical `defsrc` input is reported even if its configured action produces no output or launches a command;
- fake keys, macros, and generated OS output are excluded because they do not enter through this receiver as physical input; and
- event ordering matches the order Kanata actually processes after `collect_and_sort_events`, including its modifier-prioritization behavior.

If a client needs raw OS arrival order rather than Kanata processing order, that is a different API and would require backend-specific emission; it is outside this issue's stated need.

### 4. Advertise and demonstrate the event

- Update the `HelloOk.capabilities` list in `src/tcp_server.rs` with the selected capability identifier.
- Extend `example_tcp_client/src/main.rs` to match the new `ServerMessage`, print the physical key and state, and thereby keep the example exhaustive and compilable.
- Add the JSON shape, semantics, exclusions, ordering, repeat behavior, and capability name to the TCP server “Event Notifications” section in `docs/config.adoc`.
- Explicitly document that TCP exposes sensitive typing metadata, is only active when the user starts Kanata with a TCP listener, and should not be bound to an untrusted interface. If Open Decision 2 introduces subscriptions, document that opt-in flow as well.

## File and module changes

| Path | Change |
| --- | --- |
| `tcp_protocol/src/lib.rs` | Add the physical-input server message and serialization/round-trip tests. |
| `src/kanata/mod.rs` | Convert accepted physical `KeyEvent`s and enqueue notifications before `handle_input_event`; add focused unit tests. |
| `src/tcp_server.rs` | Advertise the capability and, if selected in Open Decision 2, maintain per-client subscription state. |
| `example_tcp_client/src/main.rs` | Handle and display the new notification; demonstrate subscription if required. |
| `docs/config.adoc` | Document the event schema, semantics, capability, security implications, and example usage. |

No platform-specific event-loop file should need modification unless testing reveals that a supported backend does not forward a particular `defsrc` event through the shared processing receiver.

## Implementation checklist

- [ ] Resolve Open Decision 1 and record the selected JSON contract in the issue/PR description.
- [ ] Resolve Open Decision 2 and determine whether connection to the TCP server alone enables physical-event delivery.
- [ ] Add the selected `ServerMessage` representation and protocol tests in `tcp_protocol/src/lib.rs`.
- [ ] Add the selected capability identifier to `HelloOk` in `src/tcp_server.rs` and cover it in a focused test or extracted capability-list test.
- [ ] Add a pure `KeyEvent`-to-notification conversion helper in `src/kanata/mod.rs`.
- [ ] Exclude wake-up/sentinel events and implement the decided repeat/tap semantics.
- [ ] Refactor the duplicated blocking/non-blocking batch handling only as much as needed to guarantee the same emission behavior in both branches.
- [ ] Enqueue the notification immediately before `handle_input_event`, using non-blocking channel behavior.
- [ ] If Open Decision 2 chooses subscription, add the client command, per-connection subscription state, routing logic, and cleanup without changing delivery of existing broadcast messages.
- [ ] Update `example_tcp_client/src/main.rs` for the new exhaustive enum match and show a readable visualization-oriented output.
- [ ] Update `docs/config.adoc` with the final wire examples and privacy/network warning.
- [ ] Run formatting, targeted tests, the normal workspace test recipe, and clippy.

## Test plan

### Protocol unit tests (`tcp_protocol/src/lib.rs`)

- Assert the exact JSON for press and release notifications.
- Round-trip deserialize each supported state.
- If repeat/tap are represented, cover their exact JSON; if filtered, cover that in engine tests instead.
- Verify existing message JSON remains unchanged.

### Engine unit tests (`src/kanata/mod.rs`)

- Convert a normal `KeyEvent` press and release and assert the physical key name/state.
- Verify the event uses the input code even when a test configuration remaps that key to a different output.
- Verify `WakeUp` / `KEY_RESERVED` produces no physical-key notification.
- Verify the decided behavior for `Repeat` and `Tap`.
- Verify generated fake-key/layout actions do not call the physical-input conversion path.
- Use a bounded channel to confirm a full notification channel does not block or fail input handling.

### Integration/simulation coverage

- Under `tcp_server` plus the existing simulated-input/output facilities, feed press/release events for a remapped key and assert that notification order is physical press, physical release while simulated OS output reflects the mapping.
- Exercise a multi-event batch containing a modifier and ordinary key to document/assert processing-order semantics.
- Connect two clients and verify the delivery behavior selected in Open Decision 2.
- Disconnect a client during notification and verify stale-client cleanup still works.

### Commands

Run the repository's established checks:

```text
cargo fmt --all -- --check
cargo test -p kanata-tcp-protocol
cargo test -p kanata --features tcp_server
just test
```

If `just test`'s final `cargo clippy --all` does not cover a non-default feature combination changed by the implementation, also run the relevant explicit `cargo clippy` invocation.

## Conventions detected

| Convention | Evidence | Confidence |
| --- | --- | --- |
| Cross-platform input behavior belongs in the shared processing loop when the original `KeyEvent` is sufficient. | `docs/design.md`; `Kanata::start_processing_loop` in `src/kanata/mod.rs`; platform event-loop files. | High |
| TCP wire types and exact serialization tests live in the protocol crate. | `tcp_protocol/src/lib.rs`. | High |
| Server events are broadcast from a processing-loop channel and use non-blocking sends. | `src/kanata/mod.rs`, including layer, push-message, and tap-hold notifications. | High |
| Physical key strings use lowercase `OsCode::Display` output. | `HoldActivated` and `TapActivated` in `src/kanata/mod.rs`; `OsCode::Display` in `parser/src/keys/mod.rs`. | High |
| New optional TCP behavior is feature-gated with `tcp_server`. | `Cargo.toml`, `src/tcp_server.rs`, and `src/kanata/mod.rs`. | High |
| TCP capabilities are advertised through `HelloOk`. | `tcp_protocol/src/lib.rs` and `src/tcp_server.rs`. | High |
| User-visible TCP changes update both the AsciiDoc reference and example client. | `docs/config.adoc`, `example_tcp_client/src/main.rs`, and recent TCP feature commits. | Medium-high |

## Decisions selected for implementation

### Physical event wire contract

Selected: one `PhysicalKey` event containing the lowercase display `key`, numeric platform-specific
OS `code`, and lowercase `state`. Press, release, repeat, and tap are preserved. This gives simple
overlay clients readable data while allowing clients that need an unambiguous identifier to use the
numeric code.

The alternatives below are retained as design history.

### Decision 1 alternatives — Physical event wire contract

**Decision:** What states and identifiers must the physical-input message expose?

No existing event represents a general `KeyEvent`. `HoldActivated`/`TapActivated` establish lowercase key names but not press/release/repeat semantics or whether a numeric code is part of a public contract.

Candidates:

1. One event with `key` and an enum-like `state` (`press`, `release`, optionally `repeat`/`tap`). This is compact and extensible, but clients must understand state values and a string key name may be ambiguous for custom/platform-specific codes.
2. Separate `PhysicalKeyPress` and `PhysicalKeyRelease` variants. This is easy for simple clients and matches the style of separate tap/hold events, but expands the enum and handles repeat/tap awkwardly.
3. One event carrying both a display `key` and numeric OS `code`, plus state. This gives clients a stable machine identifier and a readable name, but exposes platform-dependent values and requires clear portability guarantees.

The decision must also specify whether hardware `Repeat` is forwarded, normalized to `press`, or filtered, and whether mouse-wheel `KeyValue::Tap` events in `defsrc` belong in this keyboard-focused API.

### Delivery model

Selected: explicit per-connection subscription. New connections receive no physical-key events
until they send `SubscribePhysicalKeyEvents { enabled: true }`; they can disable delivery with the
same command. This is the safest fit for privacy-sensitive, high-frequency input and does not alter
traffic received by existing integrations.

The alternatives below are retained as design history.

### Decision 2 alternatives — Broadcast or subscription

**Decision:** Should every connected TCP client receive physical keystrokes automatically, or must a client explicitly subscribe?

Existing server events are broadcast to all connections, so automatic delivery follows current mechanics. However, continuous physical input is substantially higher volume and more privacy-sensitive than layer/reload/tap-hold notifications. The repository has no per-client event subscription convention to settle the choice.

Candidates:

1. Broadcast whenever the TCP server is enabled. This is the smallest change and treats starting the listener as user opt-in, but existing clients begin receiving sensitive, high-frequency messages they did not request and exhaustive Rust clients may fail on the new enum variant.
2. Add a client subscription command/capability and route physical events only to subscribed connections. This limits data and traffic to interested clients, but requires per-client state and changes the current simple broadcast architecture.
3. Add a Kanata configuration/CLI option that globally enables physical-event broadcasting. This keeps routing simple and makes consent explicit, but requires parser/argument/config plumbing and cannot distinguish clients on the same listener.

Resolve this before finalizing the routing portion of the implementation; the chosen option materially changes `src/tcp_server.rs` and possibly parser/CLI files beyond the base file list above.

## Out of scope

- A Kanata-owned overlay window or on-screen keyboard UI.
- Reporting application text, Unicode characters, or reconstructed shortcuts.
- Reporting remapped output; this issue specifically asks for the original physical input.
- A guarantee of raw hardware arrival ordering rather than Kanata's processing order.
- Exposing physical events when the TCP server feature/listener is disabled.
