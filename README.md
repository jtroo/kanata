# ktrl

**TL;DR**: ktrl is a Linux keyboard programming daemon.
It aims to aid you in the never-ending quest of achieving the ultimate keybinding setup.

You can dip your toes by remapping a few modifier keys (ex: CapLock <-> Ctrl),
or go all-in with sophisticated layering setup, dual-function keys, tap-dancing, etc...

ktrl is heavily inspired by the amazing open-source keyboard firmware project QMK.
You can think of ktrl as an attempt to re-implement QMK as a Linux daemon.

## Overview

ktrl sits right in the middle of the human-interface software stack.
It lives in userspace, between the kernel and your display server (a.k.a X11 / Wayland).

This position allows ktrl complete control over the events your keyboard generates.
These events are either transparently passed-on or transformed into ktrl's "Effects" (more on that later).

## Features

Aside from the obvious key remapping capability, ktrl provides these awesome features -

### Layers

### Tap-Hold (Dual Function Keys)

### Tap-Dancing (Multi-Tap)

### Meh and Hyper

### Audible Feedback

## Examples

### Remapping Capslock <-> Ctrl

### Home-row Modifiers

##  Limitations

- ktrl requires root privileges
