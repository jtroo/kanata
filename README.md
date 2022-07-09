# kanata

Cross-platform advanced keyboard customization.

## What does this do?

This is a software keyboard remapper for Linux and Windows. A short summary of
the features:

- cross-platform human readable configuration file
- multiple layers of key functionality
- advanced key behaviour customization (e.g. tap-hold, key sequences, unicode)

To see all of the features, see the [features](#features) section.

The most similar project is [kmonad](https://github.com/david-janssen/kmonad),
which served as the inspiration for kanata. [Here's a comparison document](./docs/kmonad_comparison.md).

## Usage

This is tested on Windows 10 and Linux (debian). See the
[releases page](https://github.com/jtroo/kanata/releases) for executables.

Using `cargo install`:

    cargo install kanata
    kanata --cfg <conf_file> # may not have permissions on Linux, see below

Build and run yourself in Linux:

    cargo build   # release optional, not really perf sensitive

    # sudo is used because kanata opens /dev/ files
    #
    # See below if you want to avoid needing sudo:
    # https://github.com/kmonad/kmonad/blob/master/doc/faq.md#linux
    sudo target/debug/kanata --cfg <conf_file>

Build and run yourself in Windows:

    cargo build   # release optional, not really perf sensitive
    target\debug\kanata --cfg <conf_file>

Sample configuration files are found in [cfg_samples](./cfg_samples). The
[simple.kbd](./cfg_samples/simple.kbd) file contains a basic configuration file
that is hopefully easy to understand but does not contain all features. The
`kanata.kbd` contains an example of all features with documentation. The latest
release assets also has a `kanata.kbd` file that is tested to work with that
release.

## Features

- Human readable configuration file. [Simple example](./cfg_samples/simple.kbd).
  [All features showcase](./cfg_samples/kanata.kbd).
- Layer switching. Change base layers between e.g. qwerty layer, dvorak layer, experimental layout layer
- Layer toggle. Toggle a layer temporarily, e.g. for a numpad layer, arrow keys layer, or symbols layer
- Tap-hold keys. Different behaviour when you tap a key vs. hold the key
  - example 1: remap caps lock to act as caps lock on tap but ctrl on hold
  - example 2: remap 'A' to act as 'A' on tap but toggle the numpad layer on hold
- Key chords. Send a key combo like Ctrl+Shift+R or Ctrl+Alt+Delete in a single keypress.
- Macros. Send a sequence of keys with optional configurable delays, e.g. `http://localhost:8080`.
- Unicode. Type any unicode character ([not guaranteed to be accepted](https://github.com/microsoft/terminal/issues/12977)
  by the target application).
- Mouse buttons. Send mouse left click, right click, and middle click events with your keyboard.
- Live reloading of the configuration for easy testing of your changes.
- Run binaries from kanata (disabled by default)

## Contributing

Contributions are welcome!

Unless explicitly stated otherwise, your contributions will be made under the
LGPLv3 license.

The keyberon project contains all of the heavy keyboard state logic, so if you
want new keyboard mapping functionality (e.g. tap-dance), it's recommended to
add it to keyberon.

[Here's a basic low-effort design doc of kanata](./docs/design.md)

## How you can help

- Try it out and let me know what you think
- Add support for MacOS

## What does the name mean?

I wanted a "k" word since this relates to keyboards. According to Wikipedia,
kanata is an indigenous Iroquoian word meaning "village" or "settlement" and is
the origin of Canada's name.

There's also PPT✧.

## Motivation

I have a few keyboards that run [QMK](https://docs.qmk.fm/#/). QMK allows the
user to customize the functionality of their keyboard to their heart's content.

One great use case of QMK is its ability map keys so that they overlap with the
home row keys but are accessible on another layer. I won't comment on
productivity, but I find this greatly helps with my keyboard comfort.

For example, these keys are on the right side of the keyboard:

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
etc.

However, QMK doesn't run everywhere. In fact, it doesn't run on **most**
hardware you can get. You can't get it to run on a laptop keyboard or any
mainstream office keyboard out there. I believe that the comfort and
empowerment QMK provides should be available to anyone with a computer on
their existing hardware, instead of having to purchase an enthusiast mechanical
keyboard. (which are admittedly very nice (I own a few) — but can be costly)

The best alternative solution that I've found for keyboards that don't run QMK is
[kmonad](https://github.com/david-janssen/kmonad). This is an excellent project
and I recommend it if you want to try something similar.

The reason for this project's existence is that kmonad is written in Haskell
and I have no idea how to begin contributing to a Haskell project. From an
outsider's perspective I think Haskell is a great language, but I really can't
wrap my head around it. One feature missing from kmonad that affects my
personal workflow is QMK's default
[tap-hold](https://docs.qmk.fm/#/tap_hold?id=tapping-force-hold) behaviour.

This project is written in Rust because Rust is my favourite programming
language and the awesome [keyberon crate](https://github.com/TeXitoi/keyberon)
exists. This project would not exist without keyberon. I was able to add my
[desired tap-hold behaviour](https://github.com/TeXitoi/keyberon/pull/85) with
not too much trouble.

I've tried compiling kmonad myself and it was quite the slog, though I was able
to get it working eventually. Comparing the process to `cargo build` though, it
was a huge contrast. I think using Rust will lower the barrier to entry for
contributions to a project like this.

## Similar Projects

- [kmonad](https://github.com/david-janssen/kmonad): The inspiration for kanata.
- [QMK](https://docs.qmk.fm/#/): Open source keyboard firmware.
- [keyberon](https://github.com/TeXitoi/keyberon): Rust `#[no_std]` library intended for keyboard firmware.
- [ktrl](https://github.com/ItayGarin/ktrl): Linux-only keyboard customizer with audio support.
- [kbremap](https://github.com/timokroeger/kbremap): Windows-only keyboard customizer with support for layers and unicode
- [xcape](https://github.com/alols/xcape): Implements tap-hold only for modifiers (Linux)
- [Space2Ctrl](https://github.com/r0adrunner/Space2Ctrl): Similar to `xcape`
- [interception tools](https://gitlab.com/interception/linux/tools): A framework for implementing tools like kanata
- [karabiner-elements](https://karabiner-elements.pqrs.org/): A mature keyboard customizer for Mac
- [capsicain](https://github.com/cajhin/capsicain): A Windows-only key remapper with driver-level key interception
- [keyd](https://github.com/rvaiya/keyd): A Linux-only key remapper very similar to kanata and kmonad
