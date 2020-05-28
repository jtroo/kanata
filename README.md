# ktrl

<p>
  <a href="https://crates.io/crates/ktrl">
    <img alt="Crates.io" src="https://img.shields.io/badge/crates.io-0.1.0-orange">
  </a>
</p>

**TL;DR**

**ktrl** is a **Linux keyboard programming daemon**.
It aims to aid you in the never-ending quest of achieving the ultimate keybinding setup.

You can dip your toes by remapping a few modifier keys (ex: CapLock <-> Ctrl).
Or you can go all-in by creating a sophisticated layering setup with dual-function keys, tap-dancing, etc...

ktrl is heavily inspired by the amazing open-source keyboard firmware project [QMK](https://docs.qmk.fm/#/).
You can think of ktrl as an attempt to re-implement QMK as a Linux daemon.

This is an **alpha** state project.
If you find any bugs or quirks please reach out to me.

## Design

ktrl sits right in the middle of the human-interface software stack.
It lives in userspace, between the kernel and your display server (a.k.a X11 / Wayland).

This position allows ktrl complete control over the events your keyboard generates.
These events are either transparently passed-on or transformed into ktrl's "Effects" (more on that later).

## Features Overview

Aside from the obvious key remapping capability, ktrl provides these awesome features -

### Layers

Although "layers" might seem like a foreign idea, it's something you're already very familiar with.
After all, you apply "layers" all the time by using modifier and function keys :)

QMK takes this mechanism and generalizes it.
Letting you design your own custom keyboard's layers!

If that sounds confusing, I encourage you to head over to [QMK's documentation about layers](https://beta.docs.qmk.fm/using-qmk/guides/keymap#keymap-and-layers).

### Tap-Hold (Dual Function Keys)

Tap-Hold keys let you do one thing when the key is pressed, and another thing when it is held.
For example, you can make your Spacebar act as normal when tapping, but serve as Ctrl when held.

### Tap-Dancing (Multi-Tap)

Tap-dancing is quite similar to Tap-Hold. The key will act in one way if tapped once,
and will act differently if tapped multiple times.

### Meh and Hyper

Again, both of these were shamelessly taken from QMK. `Meh` and `Hyper` are special modifiers
for creating keybindings that'll probably never conflict with existing ones.
That's possible since `Hyper` is equal to pressing `Ctrl+Alt+Shift+Win` and `Meh` is the same as pressing `Ctrl+Alt+Shift`.

### Audible Feedback

Ever wanted to bind your favorite 8bit tunes to your key-presses? Well, now you can!
Though, aside from making your hacking session more musical, this feature as some very practical uses as well.

For example, it can help you build new muscle-memory connections using audible feedback.
See the Capslock <-> Ctrl example below for more on that.


## Examples

### Remapping Capslock <-> Ctrl

### Home-row Modifiers

##  Limitations

- ktrl requires root privileges
