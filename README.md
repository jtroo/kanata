# ktrl (rename pending)

Rename is pending because this doesn't really look much like the original ktrl
project anymore. If you have suggestions for a new name, feel free to open an
issue or start a discussion!

## Motivation

I have a few keyboards that run [QMK](https://docs.qmk.fm/#/), which allows the
user to customize the functionality of their keyboard to their heart's content.

QMK is wonderful for being able to remap keys so that they overlap with the
home row keys, but on another layer. I won't comment on productivity, but I
find this greatly helps with my keyboard comfort.

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
etc. - personally these custamizations are not the only ones I use.

However, QMK doesn't run on everything. In fact, it doesn't run on **most**
things. You can't get it to run on a laptop keyboard, or any mainstream
office keyboard out there.

The current best solution that I've found for keyboards that don't run QMK is
[kmonad](https://github.com/david-janssen/kmonad). This is an excellent project
and I strongly recommend it if you want to use something similar right now.

The reason for this project's existence is that kmonad is written in Haskell
and I have no idea how to begin contributing to a Haskell project. One feature
missing from kmonad that affects my personal workflow is QMK's default
[tap-hold](https://docs.qmk.fm/#/tap_hold?id=tapping-force-hold) behaviour.

This project is written in Rust because Rust is my favourite programming
language and the awesome [keyberon crate](https://github.com/TeXitoi/keyberon)
exists. Keyberon is also missing the tap-hold functionality, but I actually
have some hope of being able to add it myself at some point. I've tried
compiling kmonad myself and it was quite the slog, though I was able to get it
working eventually. Compared to `cargo build` though, it was a huge contrast. I
believe Rust also has a larger talent pool, so that might be nice for getting
contributions!

## Current state

This is a rewrite of ktrl to use [keyberon](https://github.com/TeXitoi/keyberon).
Almost all of the original ktrl code has been removed, with the exception of
the Linux OS interaction. None of the original running-as-daemon stuff has been
maintained or tested, and it may have been ripped out at some point - I don't
recall.

This project is currently working with Linux using keyberon already. There are
no external configuration options available at the moment. However, this
project **can** be used in its current state if you're willing to modify the
source code directly to add your own keyberon configurations.

You would need to modify `create_mapped_keys` and `DEFAULT_LAYERS` to change
how the binary operates.

To run:

    sudo ktrl --device /dev/input/<keyboard-input>

    # e.g. this is my VMWare keyboard input
    sudo ktrl --device /dev/input/by-path/platform-i8042-serio-0-event-kbd

## Goals

- Add kmonad-style configuration
- Implement [tap hold interval](https://github.com/TeXitoi/keyberon/issues/37)
  in keyberon to achieve my desired feature parity with QMK
- Add Windows support
  - MacOS support will never be implemented by me (jtroo) because I don't own any Apple devices, but PRs are welcome.

## Similar Projects
- [kmonad](https://github.com/david-janssen/kmonad): The inspiration behind this iteration of ktrl
- [QMK](https://docs.qmk.fm/#/): An open source keyboard firmware
- [xcape](https://github.com/alols/xcape): Implements tap-hold only for modifiers (Linux)
- [Space2Ctrl](https://github.com/r0adrunner/Space2Ctrl): Similar to `xcape`
- [interception tools](https://gitlab.com/interception/linux/tools): A framework for implementing tools like ktrl
- [karabiner-elements](https://karabiner-elements.pqrs.org/): A mature keyboard customizer for Mac
