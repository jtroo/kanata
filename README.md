<h1 align="center">Kanata</h1>

<h3 align="center">
  <img
    alt="Image of a keycap with the letter K on it in pink tones"
    title="Kanata"
    height="160"
    src="assets/kanata-icon.svg"
  />
</h3>

<div align="center">
  Improve your keyboard comfort
</div>

## What does this do?

This is a cross-platform software keyboard remapper for Linux, macOS and Windows.
A short summary of the features:

- multiple layers of key functionality
- advanced key behaviour customization (e.g. tap-hold, macros, unicode)

To see all of the features, see the [configuration guide](./docs/config.adoc).

You can find pre-built binaries in the [releases page](https://github.com/jtroo/kanata/releases)
or read on for build instructions.

You can see a [list of known issues here](./docs/platform-known-issues.adoc).

### Demo

#### Demo video
[Showcase of multi-layer functionality (30s, 1.7 MB)](https://user-images.githubusercontent.com/6634136/183001314-f64a7e26-4129-4f20-bf26-7165a6e02c38.mp4).

#### Online simulator

You can check out the [online simulator](https://jtroo.github.io)
to test configuration validity and test input simulation.

## Why is this useful?

Imagine if, instead of pressing Shift to type uppercase letters, we had giant
keyboards with separate keys for lowercase and uppercase letters. I hope we can
all agree: that would be a terrible user experience!

A way to think of how Shift keys work is that they switch your input to another
layer of functionality where you now type uppercase letters and symbols
instead of lowercase letters and numbers.

What kanata allows you to do is take this alternate layer concept that Shift
keys have and apply it to any key. You can then customize what those layers do
to suit your exact needs and workflows.

## Usage

Running kanata currently does not start it in a background process.
You will need to keep the window that starts kanata running to keep kanata active.
Some tips for running kanata in the background:

- Windows: https://github.com/jtroo/kanata/discussions/193
- Linux: https://github.com/jtroo/kanata/discussions/130#discussioncomment-10227272
- Run from tray icon: [kanata-tray](https://github.com/rszyma/kanata-tray)

### Pre-built executables

See the
[releases page](https://github.com/jtroo/kanata/releases)
for executables and instructions.

### Build it yourself

This project uses the latest Rust stable toolchain. If you installed the
Rust toolchain using `rustup`, e.g. by using the instructions from the
[official website](https://www.rust-lang.org/learn/get-started),
you can get the latest stable toolchain with `rustup update stable`.

<details>
<summary>Instructions</summary>

Using `cargo install`:

    cargo install kanata

    # On Linux and macOS, this may not work without `sudo`, see below
    kanata --cfg <your_configuration_file>

Build and run yourself in Linux:

    git clone https://github.com/jtroo/kanata && cd kanata
    cargo build   # --release optional, not really perf sensitive

    # sudo is used because kanata opens /dev/ files
    #
    # See below if you want to avoid needing sudo:
    # https://github.com/jtroo/kanata/wiki/Avoid-using-sudo-on-Linux
    sudo target/debug/kanata --cfg <your_configuration_file>

Build and run yourself in Windows.

    git clone https://github.com/jtroo/kanata; cd kanata
    cargo build   # --release optional, not really perf sensitive
    target\debug\kanata --cfg <your_configuration_file>

Build and run yourself in macOS:

First install the Karabiner driver by following the macOS documentation
in the [releases page](https://github.com/jtroo/kanata/releases/).

Then you can compile and run with the instructions below:

    git clone https://github.com/jtroo/kanata && cd kanata
    cargo build   # --release optional, not really perf sensitive

    # sudo is needed to gain permission to intercept the keyboard

    sudo target/debug/kanata --cfg <your_configuration_file>

The full configuration guide is [found here](./docs/config.adoc).

Sample configuration files are found in [cfg_samples](./cfg_samples). The
[simple.kbd](./cfg_samples/simple.kbd) file contains a basic configuration file
that is hopefully easy to understand but does not contain all features. The
`kanata.kbd` contains an example of all features with documentation. The
release assets also have a `kanata.kbd` file that is tested to work with that
release. All key names can be found in the [keys module](./src/keys/mod.rs),
and you can also define your own key names.

</details>

### Feature flags

When either building yourself or using `cargo install`,
you can add feature flags that
enable functionality that is turned off by default.

<details>
<summary>Instructions</summary>

If you want to enable the `cmd` actions,
add the flag `--features cmd`.
For example:

```
cargo build --release --features cmd
cargo install --features cmd
```

On Windows,
if you want to compile a binary that uses the Interception driver,
you should add the flag `--features interception_driver`.
For example:

```
cargo build --release --features interception_driver
cargo install --features interception_driver
```

To combine multiple flags,
use a single `--features` flag
and use a comma to separate the features.
For example:

```
cargo build --release --features cmd,interception_driver
cargo install --features cmd,interception_driver
```
</details>

## Other installation methods

[![Packaging status](https://repology.org/badge/vertical-allrepos/kanata.svg)](https://repology.org/project/kanata/versions)

## Notable features

- Human-readable configuration file.
  - [Minimal example](./cfg_samples/minimal.kbd)
  - [Full guide](./docs/config.adoc)
  - [Simple example with explanations](./cfg_samples/simple.kbd)
  - [All features showcase](./cfg_samples/kanata.kbd)
- Live reloading of the configuration for easy testing of your changes.
- Multiple layers of key functionality
- Advanced actions such as tap-hold, unicode output, dynamic and static macros
- Vim-like leader sequences to execute other actions
- Optionally run a TCP server to interact with other programs
  - Other programs can respond to [layer changes or trigger layer changes](https://github.com/jtroo/kanata/issues/47)
- [Interception driver](https://web.archive.org/web/20240209172129/http://www.oblita.com/interception) support (use `kanata_wintercept.exe`)
  - Note that this issue exists, which is outside the control of this project:
    https://github.com/oblitum/Interception/issues/25

## Contributing

Contributions are welcome!

Unless explicitly stated otherwise, your contributions to kanata will be made
under the LGPL-3.0-only[*] license.

Some directories are exceptions:
- [keyberon](./keyberon): MIT License
- [interception](./interception): MIT or Apache-2.0 Licenses

[Here's a basic low-effort design doc of kanata](./docs/design.md)

[*]: https://www.gnu.org/licenses/identify-licenses-clearly.html

## How you can help

- Try it out and let me know what you think. Feel free to file an issue or
  start a discussion.
- Usability issues and unhelpful error messages are considered bugs that should
  be fixed. If you encounter any, I would be thankful if you file an issue.
- Browse the open issues and help out if you are able and/or would like to. If
  you want to try contributing, feel free to ping jtroo for some pointers.
- If you know anything about writing a keyboard driver for Windows, starting an
  open-source alternative to the Interception driver would be lovely.

## Community projects related to kanata

- [vscode-kanata](https://github.com/rszyma/vscode-kanata): Language support for kanata configuration files in VS Code
- [komokana](https://github.com/LGUG2Z/komokana): Automatic application-aware layer switching for [`komorebi`](https://github.com/LGUG2Z/komorebi) (Windows)
- [kanata-tray](https://github.com/rszyma/kanata-tray): Control kanata from a tray icon
- [OverKeys](https://github.com/conventoangelo/overkeys): Visual layer display for kanata - see your active layers and keymaps in real-time (Windows)
- Application-aware layer switching:
   - [qanata (Linux)](https://github.com/veyxov/qanata)
   - [kanawin (Windows)](https://github.com/Aqaao/kanawin)
   - [window_tools (Windows)](https://github.com/reidprichard/window_tools)
   - [nata (Linux)](https://github.com/mdSlash/nata)
   - [kanata-vk-agent (macOS)](https://github.com/devsunb/kanata-vk-agent)
   - [hyprkan (Linux)](https://github.com/mdSlash/hyprkan)

## What does the name mean?

I wanted a "k" word since this relates to keyboards. According to Wikipedia,
kanata is an indigenous Iroquoian word meaning "village" or "settlement" and is
the origin of Canada's name.

There's also PPT✧.

## Motivation

TLDR: QMK features but for any keyboard, not just fancy mechanical ones.

<details>
  <summary>Long version</summary>

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
mainstream office keyboard. I believe that the comfort and empowerment QMK
provides should be available to anyone with a computer on their existing
hardware, instead of having to purchase an enthusiast mechanical keyboard
(which are admittedly very nice — I own a few — but can be costly).

The best alternative solution that I found for keyboards that don't run QMK was
[kmonad](https://github.com/kmonad/kmonad). This is an excellent project
and I recommend it if you want to try something similar.

The reason for this project's existence is that kmonad is written in Haskell
and I have no idea how to begin contributing to a Haskell project. From an
outsider's perspective I think Haskell is a great language but I really can't
wrap my head around it. And there are a few [outstanding issues](./docs/kmonad_comparison.md)
at the time of writing that make kmonad suboptimal for my personal workflows.

This project is written in Rust because Rust is my favourite programming
language and the prior work of the awesome [keyberon crate](https://github.com/TeXitoi/keyberon)
exists.
</details>

## Similar Projects

The most similar project is [kmonad](https://github.com/kmonad/kmonad),
which served as the inspiration for kanata. [Here's a comparison document](./docs/kmonad_comparison.md).
Other similar projects:

- [QMK](https://docs.qmk.fm/#/): Open source keyboard firmware
- [keyberon](https://github.com/TeXitoi/keyberon): Rust `#[no_std]` library intended for keyboard firmware
- [ktrl](https://github.com/ItayGarin/ktrl): Linux-only keyboard customizer with layers, a TCP server, and audio support
- [kbremap](https://github.com/timokroeger/kbremap): Windows-only keyboard customizer with layers and unicode
- [xcape](https://github.com/alols/xcape): Linux-only tap-hold modifiers
- [karabiner-elements](https://karabiner-elements.pqrs.org/): Mac-only keyboard customizer
- [capsicain](https://github.com/cajhin/capsicain): Windows-only key remapper with driver-level key interception
- [keyd](https://github.com/rvaiya/keyd): Linux-only key remapper very similar to QMK, kmonad, and kanata
- [xremap](https://github.com/k0kubun/xremap): Linux-only application-aware key remapper inspired more by Emacs key sequences vs. QMK layers/Vim modes
- [keymapper](https://github.com/houmain/keymapper): Context-aware cross-platform key remapper with a different transformation model (Linux, Windows, Mac)
- [mouseless](https://github.com/jbensmann/mouseless): Linux-only mouse-focused key remapper that also has layers, key combo and tap-hold capabilities

### Why the list?

While kanata is the best tool for some, it may not be the best tool for
you. I'm happy to introduce you to tools that may better suit your needs. This
list is also useful as reference/inspiration for functionality that could be
added to kanata.

## Donations/Support?

The author (jtroo) will not accept monetary donations for work on kanata.
Please instead donate your time and/or money to charity.

Some links are below. These links are provided for learning and as interesting
reads. They are **not** an endorsement.

- https://www.effectivealtruism.org/
- https://www.givewell.org/
