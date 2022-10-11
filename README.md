# kanata

Improve keyboard comfort and usability with advanced customization.

## What does this do?

This is a software keyboard remapper for Linux and Windows. A short summary of
the features:

- multiple layers of key functionality
- advanced key behaviour customization (e.g. tap-hold, key sequences, unicode)
- cross-platform human readable configuration file

To see all of the features, see the [features](#features) section.

[Here's a demo video showcasing multi-layer functionality (30s, 1.7 MB)](https://user-images.githubusercontent.com/6634136/183001314-f64a7e26-4129-4f20-bf26-7165a6e02c38.mp4).

The most similar project is [kmonad](https://github.com/david-janssen/kmonad),
which served as the inspiration for kanata. [Here's a comparison document](./docs/kmonad_comparison.md).

## Why is this useful?

Imagine if, instead of pressing Shift to type uppercase letters, we had giant
keyboards with separate keys for lowercase and uppercase letters. I hope we can
all agree: that would be a terrible user experience!

A way to think of how Shift keys work is that they switch your input to another
layer of functionality where you now type uppercase letters and symbols
instead of lowercase letters and numbers.

What kanata allows you to do is take this alternate layer concept that Shift
keys add and apply it to any key. You can then customize what those layers do to
suit your exact needs and workflows.

## Usage

This is tested by jtroo on Windows 10 and Debian 10. See the
[releases page](https://github.com/jtroo/kanata/releases) for executables.

Using `cargo install`:

    cargo install kanata

    # may not have permissions without sudo on Linux, see below
    kanata --cfg <conf_file>

Build and run yourself in Linux:

    cargo build   # release optional, not really perf sensitive

    # sudo is used because kanata opens /dev/ files
    #
    # See below if you want to avoid needing sudo:
    # https://github.com/jtroo/kanata/wiki/Avoid-using-sudo-on-Linux
    sudo target/debug/kanata --cfg <conf_file>

Build and run yourself in Windows:

    cargo build   # release optional, not really perf sensitive
    target\debug\kanata --cfg <conf_file>

Sample configuration files are found in [cfg_samples](./cfg_samples). The
[simple.kbd](./cfg_samples/simple.kbd) file contains a basic configuration file
that is hopefully easy to understand but does not contain all features. The
`kanata.kbd` contains an example of all features with documentation. The latest
release assets also has a `kanata.kbd` file that is tested to work with that
release. All key names can be found in the [keys module](./src/keys/mod.rs).

## Other installation methods

[![Packaging status](https://repology.org/badge/vertical-allrepos/kanata.svg)](https://repology.org/project/kanata/versions)

## Features

- Human readable configuration file.
  - [Minimal example](./cfg_samples/minimal.kbd)
  - [Full guide](./docs/config.adoc)
  - [Simple example with explanations](./cfg_samples/simple.kbd)
  - [All features showcase](./cfg_samples/kanata.kbd)
- Press (Left Control+Space+Escape) to terminate kanata at any time in case you've messed up your config.
- Key chords. Send a key combo like Ctrl+Shift+R or Ctrl+Alt+Delete in a single keypress.
- Mouse buttons. Send mouse left click, right click, and middle click events with your keyboard.
- One-shot keys. Activate a modifier like `LShift` for exactly one subsequent keypress.
- Layer switching. Change base layers between e.g. qwerty layer, dvorak layer, experimental layout layer
- Layer toggle. Toggle a layer temporarily, e.g. for a numpad layer, arrow keys layer, or symbols layer
- Tap-hold keys. Different behaviour when you tap a key vs. hold the key
  - example 1: remap caps lock to act as caps lock on tap but ctrl on hold
  - example 2: remap 'A' to act as 'A' on tap but toggle the numpad layer on hold
- Tap-dance. Perform different actions with the same key depending on how many rapid taps were done.
- Macros. Send a sequence of keys with optional configurable delays, e.g. `http://localhost:8080`.
- Unicode. Type any unicode character ([not guaranteed to be accepted](https://github.com/microsoft/terminal/issues/12977)
  by the target application).
- Optionally run a TCP server to interact with other programs
  - Other programs can respond to [layer changes or trigger layer changes](https://github.com/jtroo/kanata/issues/47)
- Vim-like leader sequences to execute other actions
- Live reloading of the configuration for easy testing of your changes.
- [Interception driver](http://www.oblita.com/interception) support (use `kanata_wintercept.exe`)
- Run binaries from kanata (disabled by default)

## Contributing

Contributions are welcome!

Unless explicitly stated otherwise, your contributions will be made under the
LGPL-3.0-only[*] license.

[Here's a basic low-effort design doc of kanata](./docs/design.md)

[*]: https://www.gnu.org/licenses/identify-licenses-clearly.html

## How you can help

- Try it out and let me know what you think. Feel free to file an issue or
  start a discussion.
- Browse the open issues and help out if you would like to

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
etc. Personally my main motivator is comfort due to a repetitive strain injury
in the past.

However, QMK doesn't run everywhere. In fact, it doesn't run on **most**
hardware you can get. You can't get it to run on a laptop keyboard or any
mainstream office keyboard out there. I believe that the comfort and
empowerment QMK provides should be available to anyone with a computer on
their existing hardware, instead of having to purchase an enthusiast mechanical
keyboard. (which are admittedly very nice — I own a few — but can be costly)

The best alternative solution that I've found for keyboards that don't run QMK is
[kmonad](https://github.com/david-janssen/kmonad). This is an excellent project
and I recommend it if you want to try something similar.

The reason for this project's existence is that kmonad is written in Haskell
and I have no idea how to begin contributing to a Haskell project. From an
outsider's perspective I think Haskell is a great language but I really can't
wrap my head around it. And there are a few [outstanding issues](./docs/kmonad_comparison.md)
at the time of writing that make kmonad suboptimal for my personal workflows.

This project is written in Rust because Rust is my favourite programming
language and the prior work of the awesome [keyberon crate](https://github.com/TeXitoi/keyberon)
exists.

## Similar Projects

- [kmonad](https://github.com/david-janssen/kmonad): The inspiration for kanata (Linux, Windows, Mac)
- [QMK](https://docs.qmk.fm/#/): Open source keyboard firmware
- [keyberon](https://github.com/TeXitoi/keyberon): Rust `#[no_std]` library intended for keyboard firmware
- [ktrl](https://github.com/ItayGarin/ktrl): Linux-only keyboard customizer with layers, a TCP server, and audio support
- [kbremap](https://github.com/timokroeger/kbremap): Windows-only keyboard customizer with layers and unicode
- [xcape](https://github.com/alols/xcape): Linux-only tap-hold modifiers
- [karabiner-elements](https://karabiner-elements.pqrs.org/): Mac-only keyboard customizer
- [capsicain](https://github.com/cajhin/capsicain): Windows-only key remapper with driver-level key interception
- [keyd](https://github.com/rvaiya/keyd): Linux-only key remapper very similar to QMK, kmonad, and kanata
- [xremap](https://github.com/k0kubun/xremap): Linux-only application-aware key remapper inspired more by Emacs key sequences vs. QMK layers/Vim modes

### Why the list?

While kanata is the best tool for me (jtroo), it may not be the best tool for
you. I'm happy to introduce you to tools that may better suit your needs. This
list is also useful as reference/inspiration for functionality that could be
added to kanata.
