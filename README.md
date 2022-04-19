# ktrl

Readme TODO!

Rewrite of ktrl to use [keyberon](https://github.com/TeXitoi/keyberon) in progress.

To run:

    sudo ktrl --device /dev/input/<keyboard-input>

    # e.g. this is my VMWare keyboard input
    sudo ktrl --device /dev/input/by-path/platform-i8042-serio-0-event-kbd

## Goals

- Add Windows support
  - MacOS support will never be implemented by me (jtroo) because I don't own any Apple devices, but PRs are welcome.
- Add kmonad-style configuration

## Similar Projects
- [QMK](https://docs.qmk.fm/#/): An open source keyboard firmware (ktrl's inspiration)
- [kmonad](https://github.com/david-janssen/kmonad): Very similar to ktrl (written in Haskell)
- [xcape](https://github.com/alols/xcape): Implements tap-hold only for modifiers (Linux)
- [Space2Ctrl](https://github.com/r0adrunner/Space2Ctrl): Similar to `xcape`
- [interception tools](https://gitlab.com/interception/linux/tools): A framework for implementing tools like ktrl
- [karabiner-elements](https://karabiner-elements.pqrs.org/): A mature keyboard customizer for Mac
