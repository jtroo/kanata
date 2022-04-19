# ktrl

Rewrite of ktrl to use [keyberon](https://github.com/TeXitoi/keyberon).
This code is currently working with Linux using keyberon already. None of the
original running-as-daemon stuff has been maintained or tested, and it may have
been ripped out, not sure.

There are no external configuration options available at the moment. However,
this project **can** be used in its current state if you're willing to modify
the source code directly to add your own keyberon configurations.

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
