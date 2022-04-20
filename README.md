# ktrl (rename pending)

Rename is pending because this doesn't really look much like the original ktrl
project anymore. If you have suggestions for a new name, feel free to open an
issue or start a discussion!

## Current state

This is a rewrite of the old ktrl project to use
[keyberon](https://github.com/TeXitoi/keyberon). Almost all of the original
ktrl code has been removed, with the exception of the Linux OS interaction.
None of the original running-as-daemon code has been maintained/tested, and it
may have been ripped out at some point - I don't recall.

This project is working with Linux (very much untested). There are no external
configuration options available at the moment. However, this project **can** be
used in its current state if you're willing to modify the source code to add
your own keyberon configurations.

You would need to modify `create_mapped_keys` and `DEFAULT_LAYERS` to change
how the binary operates.

To run:

    sudo ktrl --device /dev/input/<keyboard-input>

    # e.g. this is my VMWare keyboard input
    sudo ktrl --device /dev/input/by-path/platform-i8042-serio-0-event-kbd

**WARNING:** I am not a keyboard expert, neither for the USB protocol or the OS
interface. There may be some incorrect mappings for the more obscure keys
between keyberon `KeyCode` and ktrl's `OsCode` in:

    impl From<KeyCode> for OsCode
    impl From<OsCode> for KeyCode

## Goals

- Add kmonad-style [configuration](https://github.com/jtroo/ktrl/issues/1)
- Add [Windows support](https://github.com/jtroo/ktrl/issues/2)
  - MacOS support will never be implemented by me (jtroo) because I don't own
    any Apple devices, but PRs are welcome.

## Contributing

While I plan to implement all of the goals at some point, contributions are
welcome!

The keyberon project contains all of the heavy logic, so if you want new
keyboard mapping functionality, it will need to be implemented in keyberon
first.

## Motivation

I have a few keyboards that run [QMK](https://docs.qmk.fm/#/). QMK allows the
user to customize the functionality of their keyboard to their heart's content.

One great use case of QMK is its ability map keys so that they overlap with the
home row keys but are accessible on another layer. I won't comment on
productivity, but I find this greatly helps with my keyboard comfort.

For example, these keys are on my right hand:

    7 8 9
    u i o
    j k l
    m , .

On one layer I have arrow keys in the same position, and on another layer I
have a numpad.

    arrows:       numpad:
    - - -         7 8 9
    - ↑ -         4 5 6
    ← ↓ →         1 2 3
    - - -         0 * .

One could add as many customizations as one likes to improve comfort, speed,
etc. Personally these customizations are not the only ones I use.

However, QMK doesn't run everywhere. In fact, it doesn't run on **most**
hardware you can get. You can't get it to run on a laptop keyboard or any
mainstream office keyboard out there. I believe that the comfort and
empowerment QMK provides should be available to anyone with a computer on
their existing hardware, instead of having to purchase an enthusiast mechanical
keyboard. (which are admittedly very nice (I own a few) — but can be costly)

The current best solution that I've found for keyboards that don't run QMK is
[kmonad](https://github.com/david-janssen/kmonad). This is an excellent project
and I strongly recommend it if you want to use something similar right now.

The reason for this project's existence is that kmonad is written in Haskell
and I have no idea how to begin contributing to a Haskell project. From an
outsider's perspective I think Haskell is a great language, but I really can't
wrap my head around it. One feature missing from kmonad that affects my
personal workflow is QMK's default
[tap-hold](https://docs.qmk.fm/#/tap_hold?id=tapping-force-hold) behaviour.

This project is written in Rust because Rust is my favourite programming
language and the awesome [keyberon crate](https://github.com/TeXitoi/keyberon)
exists. Keyberon is also currently missing the tap-hold functionality, but I
actually have some hope of being able to add it myself at some point.

I've tried compiling kmonad myself and it was quite the slog, though I was able
to get it working eventually. Comparing the process to `cargo build` though, it
was a huge contrast. I believe using Rust will lower the barrier to entry for
contributions to a project like this.

## Similar Projects
- [kmonad](https://github.com/david-janssen/kmonad): The inspiration behind this iteration of ktrl
- [QMK](https://docs.qmk.fm/#/): An open source keyboard firmware
- [xcape](https://github.com/alols/xcape): Implements tap-hold only for modifiers (Linux)
- [Space2Ctrl](https://github.com/r0adrunner/Space2Ctrl): Similar to `xcape`
- [interception tools](https://gitlab.com/interception/linux/tools): A framework for implementing tools like ktrl
- [karabiner-elements](https://karabiner-elements.pqrs.org/): A mature keyboard customizer for Mac
