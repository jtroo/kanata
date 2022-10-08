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

It should be noted that the `defsrc` entry is treated as a long list; the
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
