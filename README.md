# ktrl

**TL;DR**

**ktrl** is a **Linux keyboard programming daemon**.
It aims to aid you in the never-ending quest of achieving the ultimate keybinding setup.

You can dip your toes by remapping a few modifier keys (ex: CapLock <-> Ctrl).
Or you can go all-in by creating sophisticated layering setup with dual-function keys, tap-dancing, etc...

ktrl is heavily inspired by the amazing open-source keyboard firmware project QMK.
You can think of ktrl as an attempt to re-implement QMK as a Linux daemon.

This is an **alpha** state project.
If you find any bugs or quirks please reach out to me.

## Overview

ktrl sits right in the middle of the human-interface software stack.
It lives in userspace, between the kernel and your display server (a.k.a X11 / Wayland).

This position allows ktrl complete control over the events your keyboard generates.
These events are either transparently passed-on or transformed into ktrl's "Effects" (more on that later).

## Features

Aside from the obvious key remapping capability, ktrl provides these awesome features -

### Layers

Although "layers" might seem like a foreign idea, it's actually something you're already very familiar with.
After all, you apply "layers" all the time by using modifier and function keys :)

QMK takes this mechanism and generalizes it.
Letting you design your own custom keyboard's layers!

If that sounds confusing, I encourage you to head over to QMK's documentation about layers.

### Tap-Hold (Dual Function Keys)

### Tap-Dancing (Multi-Tap)

### Meh and Hyper

### Audible Feedback

## Examples

### Remapping Capslock <-> Ctrl

### Home-row Modifiers

##  Limitations

- ktrl requires root privileges
