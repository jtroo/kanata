# ktrl (rename pending)

Rename is pending because this doesn't really look much like the original ktrl
project anymore. If you have suggestions for a new name, feel free to open an
issue or start a discussion!

## What does this do?

This is a software keyboard remapper. Some notable features are multiple layers
of key functionality and differing key behaviour depending on if you quickly
tap the key or hold it down.

## Usage

This currently works on Linux only, though Windows is planned in the near
future.

To run:

    sudo ktrl --cfg <conf_file>

A sample configuration file is found in [cfg_samples](./cfg_samples/jtroo.kbd).

## How you can help

- Try it out and let me know what you think
- File issues and contribute PRs
- Suggest a name
- Implement Windows support 😉
- Improve `get_root_exprs` and `parse_expr` (I'm no expert in parsing)
- Add to `str_to_oscode`. This function is only contains enough cases for my
  own personal configuration.
- I am not a keyboard expert, neither for the USB protocol nor the OS interface.
  There may be some incorrect mappings for the more obscure keys between keyberon
  `KeyCode` and ktrl's `OsCode` in:
  ```rust
  impl From<KeyCode> for OsCode
  impl From<OsCode> for KeyCode
  ```

## Goals

- Add [Windows support](https://github.com/jtroo/ktrl/issues/2)
- MacOS support will never be implemented by me (jtroo) because I don't own
  any Apple devices, but PRs are welcome.

## Contributing

Contributions are welcome!

Unless explicitly stated otherwise, your contributions will be made under the
LGPLv3 license.

The keyberon project contains all of the heavy keyboard state logic, so if you
want new keyboard mapping functionality, it's strongly recommended to
contribute to keyberon first.

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
- [kmonad](https://github.com/david-janssen/kmonad): The inspiration behind this iteration of ktrl
- [QMK](https://docs.qmk.fm/#/): An open source keyboard firmware
- [xcape](https://github.com/alols/xcape): Implements tap-hold only for modifiers (Linux)
- [Space2Ctrl](https://github.com/r0adrunner/Space2Ctrl): Similar to `xcape`
- [interception tools](https://gitlab.com/interception/linux/tools): A framework for implementing tools like ktrl
- [karabiner-elements](https://karabiner-elements.pqrs.org/): A mature keyboard customizer for Mac
