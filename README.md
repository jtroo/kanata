# ktrl

<p>
  <a href="https://crates.io/crates/ktrl">
    <img alt="Crates.io" src="https://img.shields.io/badge/crates.io-0.1.0-orange">
  </a>
</p>

**TL;DR**

**ktrl** is a **Linux keyboard programming daemon**.
It aims to aid you in the never-ending quest of achieving the ultimate keybinding setup.

You can dip your toes by remapping a few modifier keys (e.g `CapLock` to `Ctrl`).
Or you can go all-in by creating a sophisticated layering setup with dual-function keys, tap-dancing, etc...

ktrl is heavily inspired by the amazing open-source keyboard firmware project [QMK](https://docs.qmk.fm/#/).
You can think of ktrl as an attempt to re-implement QMK as a Linux daemon.

This is an **alpha** state project.
If you find any bugs or quirks please reach out to me.

## Intro

ktrl sits right in the middle of the human-interface software stack.
It lives in userspace, between the kernel and your display server (a.k.a X11 / Wayland).

This position allows ktrl complete control over the events your keyboard generates.
These events are either transparently passed-on or transformed into ktrl's "Effects" (more on that later).

## Features Overview

Aside from the obvious key remapping capability,
here's a taste of some of the major things you can do with ktrl -

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

## Installation

#### Getting the Executable

Start off by grabbing the main `ktrl` executable. Here's how you do that -

```
cargo install ktrl
```

#### Setting up ktrl's Assets and Config

Now, it's time to decide where you'd like ktrl's assets and config to live.

By default, ktrl will assume you've placed these under `/opt/ktrl`.
Specifically, `/opt/ktrl/cfg.ron` and `/opt/ktrl/assets`.
Though, you can override these defaults with the `--cfg` and `--assets` cli arguments.

To set-up the defaults, you can follow these steps -

```
git clone https://github.com/itaygarin/ktrl
cd ktrl
sudo mkdir /opt/ktrl
sudo cp -r ./assets /opt/ktrl
sudo cp examples/cfg.ron /opt/ktrl
```

#### Locating your Keyboard's input device

For ktrl to work, you have to supply it with a path to your keyboard's input device.
Input devices reside in the `/dev/input` directory.

Linux provides two convenient symlinks-filled directories to make the search process easier.
These directories are `/dev/input/by-id` and `/dev/input/by-path`.

Within these two directories keyboard devices usually have a `-kbd` suffix.
For example, in my laptop, the path is `/dev/input/by-path/platform-i8042-serio-0-event-kbd`.


#### Setting up ktrl as a Service (Optional)

ktrl is a daemon that's designed to run as a background process.
Therefore, you might want to set it up as a service. Feel free
to skip this step if you just want to experiment with it.

Creating a service will vary from distro to distro,
though, here are some basic steps that'll get you started on `systemd` based systems -

```
cd ktrl
edit ktrl.service # change your username and device path
sudo cp ktrl.service /etc/systemd/system
sudo systemctl daemon-reload
sudo systemctl start ktrl.service
```

## Configuration

Finally, we get to the cool part!
Though, let's briefly go over ktrl's config primitives before assembling our first config file

### Primitives

#### Input Event Codes

ktrl uses Linux's input-event-codes everywhere.
The full list can be found either in Linux's codebase [here](https://github.com/torvalds/linux/blob/master/include/uapi/linux/input-event-codes.h#L75)
or under ktrl's [KeyCode](https://github.com/ItayGarin/ktrl/blob/master/src/keys.rs) enum.

Specifically, ktrl uses a subset of the event codes that describe keyboard keys.
E.g `KEY_A` and `KEY_LEFTCTRL` describe the 'A' and Left-Ctrl keys.

#### Actions

Within a layer, we map a source key (ex: `KEY_A`) into an `Action`.
Actions describe the **physical input** movements you'll apply to the source key.
E.g A `TapHold` describes a **tap** and a **hold**.

##### Actions List

- `Tap`: This is the default keyboard action. Use for simple key remappings.
- `TapHold`: Attach different `Effect`s for a tap and and hold.
- `TapDance`: Attach different `Effect`s for a tap and for multiple taps.

#### Effects

An `Action` will contain one or more `Effect`s.
These are the **virtual output** effects that'll manifest following the action.
E.g Playing a sound, toggling a layer, sending a key sequence, etc...

##### Effects List

- `NoOp`: As the name suggests. This won't do anything.
- `Key`: This is the default effect you're "used to".
- `KeySticky`: Once pressed, the key will remain active until pressed again (like Capslock).
- `KeySeq`: Outputs multiple keys at once. E.g `Meh` and `Hyper` are `KeySeq`s
- `Meh`: A shorthand for `KeySeq(KEY_LEFTCTRL, KEY_LEFTALT, KEY_LEFTSHIFT)`
- `Hyper`: A shorthand for `KeySeq(KEY_LEFTCTRL, KEY_LEFTALT, KEY_LEFTSHIFT, KEY_LEFTMETA)`
- `ToggleLayer`: When pressed, either turns on or off a layer.
- `MomentaryLayer`: While pressed, the relevant layer will remain active
- `Sound`: Plays one of the pre-built sounds
- `SoundEx`: Plays a custom sound provided by you.
- `Multi`: Lets you combine all the above effects. E.g `Multi([Sound(Sticky), KeySticky(KEY_LEFTCTRL)])`

### Configuration File Format

ktrl uses the wonderful [ron](https://github.com/ron-rs/ron) (Rust Object Notation) to make serializing
configuration much easier. The format should be pretty intuitive, though please refer to the supplied [cfg.ron](https://github.com/ItayGarin/ktrl/blob/master/examples/cfg.ron) for a practical example.

## Examples

### Remapping `Ctrl` to `Capslock`

This is probably one of most effective and simple you can make right now.
You're pinky finger will much appreciate this change in the long-run.

Doing this with ktrl is easy. In one of your layers, add the following -

```
KEY_CAPSLOCK:  Tap(Key(KEY_LEFTCTRL)),
KEY_LEFTCTRL:  Key(KEY_LEFTCTRL)),
```

Though, let's make this more interesting, shall we?

To make the transition smoother, let's add an error sound effect to the left Ctrl.
This'll remind you you're doing something wrong -

```
KEY_CAPSLOCK:  Tap(Key(KEY_LEFTCTRL)),
KEY_LEFTCTRL:  Tap(Multi([Key(KEY_LEFTCTRL), Sound(Error)])),
```

Ah, much better!

Of course, you can also go cold turkey and only leave the sound effect. Like so -

```
KEY_CAPSLOCK:  Tap(Key(KEY_LEFTCTRL)),
KEY_LEFTCTRL:  Tap(Sound(Error)),
```

### Home-row Modifiers

##  Limitations

- ktrl requires root privileges
