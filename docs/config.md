# Configuration

This document describes how to create a kanata configuration file. The kanata
configuration file will determine your keyboard behaviour upon running kanata.

The configuration file uses S-expression syntax from Lisps. If you are not
familiar with any Lisp-like programming language, do not be too worried. This
document will hopefully be a sufficient guide to help you customize your
keyboard behaviour to your exact liking.

If you have any questions or confusions, feel free to file an issue or start a
discussion.

## Comments

You can add comments to your configuration file. Comments are prefixed with two
semicolons. E.g:

```
;; This is a comment in a kanata configuration file.
;; There is no special syntax for multi-line comments at this time.
;; Comments will be ignored and are intended for you to help understand your
;; own configuration when reading it later.
```

## Required configuration entries

### defcfg

Your configuration file must have a `defcfg` entry.

It can be empty but there are optional entries that can change kanata's
behaviour that will be described later.

E.g. place this in your configuration file:

```
(defcfg)
```

### defsrc

Your configuration file must have exactly one `defsrc` entry. This defines the
order of keys that the configuration entries `deflayer`s will operate on.

A `defsrc` entry is composed of `(defsrc` followed by key names that are
separated by whitespace.

It should be noted that the `defsrc` entry is treated as a long sequence; the
amount of spaces, tabs, and newlines are not relevant. You may use spaces,
tabs, or newlines however you like to format `defsrc` to your liking.

An example `defsrc` containing the standard QWERTY keyboard keys, as an
approximately 60% layout.

```
(defsrc
  grv  1    2    3    4    5    6    7    8    9    0    -    =    bspc
  tab  q    w    e    r    t    y    u    i    o    p    [    ]    \
  caps a    s    d    f    g    h    j    k    l    ;    '    ret
  lsft z    x    c    v    b    n    m    ,    .    /    rsft
  lctl lmet lalt           spc            ralt rmet rctl
)
```

### deflayer

Your configuration file must have at least one `deflayer` entry. This defines
how each physical key mapped in `defsrc` behaves when `kanata` runs.

The first layer defined in your configuration file will be the starting layer
when kanata runs. Other layers can be either toggled or switched to using
special actions which will be explained later.

An example of remapping of QWERTY to the Dvorak layout would be:

```
(defsrc
  grv  1    2    3    4    5    6    7    8    9    0    -    =    bspc
  tab  q    w    e    r    t    y    u    i    o    p    [    ]    \
  caps a    s    d    f    g    h    j    k    l    ;    '    ret
  lsft z    x    c    v    b    n    m    ,    .    /    rsft
  lctl lmet lalt           spc            ralt rmet rctl
)

(deflayer dvorak
  grv  1    2    3    4    5    6    7    8    9    0    [    ]    bspc
  tab  '    ,    .    p    y    f    g    c    r    l    /    =    \
  caps a    o    e    u    i    d    h    t    n    s    -    ret
  lsft ;    q    j    k    x    b    m    w    v    z    rsft
  lctl lmet lalt           spc            ralt rmet rctl
)
```

### Review of required configuration entries

If you're reading in order, you have now seen all of the required entries:

- `defcfg`
- `defsrc`
- `deflayer`

An example minimal configuration is:

```
(defcfg)

(defsrc a b c)

(deflayer start 1 2 3)
```

This will make kanata remap your `a b c` keys to `1 2 3`, which is almost
certainly undesirable, but this will run.

## Special actions

The special actions kanata provides are what make it truly customizable.

### defalias

Before seeing the actual special actions, you should first learn about aliases.
Using the `defalias` configuration item, you can introduce a shorter form of
another action to keep alignment in `deflayer` entries clean.

Similar to how `defcfg` works, `defalias` reads pairs of items in a sequence
where the first item in the pair is the alias name and the second item is the
action it can be substituted for. However, unlike before, the second item in
`defalias` is may be a "list" as opposed to a single string like it was in
`defcfg`.

A list is a sequence of strings separated by whitespace, surrounded by
parentheses. All of the configuration items we've looked at so far are lists;
`defalias` is where we'll first see nested lists in this guide.

Example:

```
(defalias
  ;; tap for caps lock, hold for left control
  ;; tap-hold-release will be explained later.
  cap (tap-hold-release 200 200 caps lctl)
)
```

This alias can be used in `deflayer` as a substitute for the long special
action. The alias name is prefixed with `@` to signify that an alias should
be substituted.

```
(deflayer example
  @cap a s d f
)
```

You can choose to put special actions without aliasing them right into
`deflayer`. However for the long actions, it is recommended not to do so, in
order to keep a nice visual alignment for ease of reading for your
configuration.

Example:

```
(deflayer example
  ;; this is equivalent to the previous example
  (tap-hold-release 200 200 caps lctl) a s d f
)
```

You may have multiple `defalias` entries and multiple aliases within a single
`defalias`.

Example:

```
(defalias one (tap-hold-release 200 200 caps lctl))
(defalias two (tap-hold-release 200 200 esc lctl))
(defalias
  3 (tap-hold-release 200 200 home lalt)
  4 (tap-hold-release 200 200 end ralt)
)
```

### layer-switch

This action allows you to switch to another "base" layer. This is permanent
until a `layer-switch` to another layer is activated. The concept of a base
layer makes more sense when looking at the next action you'll see:
`layer-while-held`.

This action accepts a single subsequent string which must be a defined layer
name from a `deflayer` entry.

Example:

```
(defalias dvk (layer-switch dvorak))
```

### layer-while-held

This action allows you to temporarily change to another layer while the key
remains held. When the key is released, you go back to the currently active
"base" layer.

This action accepts a single subsequent string which must be a defined layer
name from a `deflayer` entry.

Example:

```
(defalias nav (layer-while-held navigation))
```

You may also use `layer-toggle` in place of `layer-while-held`; they behave
exactly the same. The `layer-toggle` name is slightly shorter but is a bit
inaccurate with regards to its meaning.

### Transparent key

If you use a single underscore for a key `_` then it acts as a "transparent"
key. The behaviour depends if `_` is on a base layer or a while-held layer.
When `_` is pressed on the active base layer, the key will default to the
corresponding `defsrc` key. If `_` is pressed on the active while-held layer,
the base layer's behaviour will activate.

Example:

```
(defsrc
  a b c
)
(deflayer remap-only-c
  _ _ d
)
```

## Optional defcfg entries

There are a few `defcfg` entries that are used to customize various kanata
behaviours.

### process-unmapped-keys

Enabling this configuration makes kanata process keys that are not in defsrc.
This is useful if you are only mapping a few keys in defsrc instead of most of
the keys on your keyboard.

Without this, the special actions (which are explained later)
`tap-hold-release` and `tap-hold-press` actions will not activate for keys that
are not in defsrc.

This is disabled by default. The reason this is not enabled by default is
because some keys may not work correctly if they are intercepted. For example,
see the [windows-altgr](#windows-only-windows-altgr) configuration item below.

Example:

```
(defcfg
  process-unmapped-keys yes
)
```

### danger-enable-cmd

This configuration item can be used to enable the `cmd` special action in your
configuration. This action allows kanata to execute programs with arguments
passed to them.

This requires using a kanata program that is compiled with the `cmd` action
enabled so that if you choose to, there is no way for kanata to execute
arbitrary binaries even if you're testing out a configuration with
`danger-enable-cmd` enabled.

This configuration is disabled by default and can be enabled by giving it the
value `yes`.

Example:

```
(defcfg
  danger-enable-cmd yes
)
```

### Linux only: linux-dev

By default, kanata will try to detect which input devices are keyboards and try
to intercept them all. However, you may specify exact keyboard devices from the
`/dev/input` directories using the `linux-dev` configuration.

Example:

```
(defcfg
  linux-dev /dev/input/by-path/platform-i8042-serio-0-event-kbd
)
```

If you want to specify multiple keyboards, you can separate the paths with a
colon `:`. Example:

```
(defcfg
  linux-dev /dev/input/dev1:/dev/input/dev2
)
```

Due to using the colon to separate devices, if you have a device with a colon
in its file name, you should escape those colons with backslashes:

```
(defcfg
  linux-dev /dev/input/path-to\:device
)
```

### Windows only: windows-altgr

There is an optional configuration entry for Windows to help mitigate strange
behaviour of AltGr (ralt) if you're using that key in your defsrc. You can use
one of the listed values to change what kanata does with the key:

- `cancel-lctl-release`
  - This will remove the `lctl` press that is generated alonside `ralt`
- `add-lctl-release`
  - This adds an `lctl` release when `ralt` is released

Example:

```
(defcfg
  windows-altgr add-lctl-release
)
```

For more context, see: https://github.com/jtroo/kanata/issues/55.

NOTE: even with these workarounds, putting lctl+ralt in your defsrc may not
work too well with other applications that use keyboard interception. Known
applications with issues: GWSL/VcXsrv

### Using multiple defcfg entries

The `defcfg` entry is treated as a list with pairs of items. For example:

```
(defcfg a 1 b 2)
```

This will be treated as configuration `a` having value `1` and configuration
`b` having value `2`.

An example defcfg containing all of the configuration items is shown below. It
should be noted that configuration items that are Linux-only or Windows-only
will be ignored when used on the non-applicable operating system.

```
(defcfg
  process-unmapped-keys yes
  danger-enable-cmd yes
  linux-dev /dev/input/dev1:/dev/input/dev2
  windows-altgr add-lctl-release
)
```

## Advanced

### defseq

### deffakekeys
