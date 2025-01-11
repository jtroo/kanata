## Configuration guide

<!-- NOTE: GitHub release doc seems to not support multiline paragraph joining as opposed to other places markdown is used in GitHub. Keep paragraphs on one line in this file, as ugly as it is to do so. -->

Link to the appropriate configuration guide version: [guide link TODO: FIX LINK](https://github.com/jtroo/kanata/blob/FIXME/docs/config.adoc).

## Changelog (since <TODO: previous_version_here>)

<details>
<summary>Change log</summary>
* TODO: fill this out
</details>

## Sample configuration file

The attached `kanata.kbd` file is tested to work with the current version. The one in the `main` branch of the repository may have extra features that are not supported in this release.

## Windows

<details>
<summary>Instructions</summary>

**NOTE:** All Windows binaries are compiled for x86-64 architectures only.

Download `kanata.exe`. Optionally, download `kanata.kbd`. With the two files in the same directory, you can double-click the `exe` to start kanata. Kanata does not start a background process, so the window needs to stay open after startup. See [this discussion](https://github.com/jtroo/kanata/discussions/193) for tips to run kanata in the background.

You need to run `kanata.exe` via `cmd` or `powershell` to use a different configuration file:

`kanata.exe --cfg <cfg_file>`

---

**NOTE:** The `kanata_winIOv2.exe` variant contains an experimental breaking change that fixes [an issue](https://github.com/jtroo/kanata/issues/152) where the Windows LLHOOK+SendInput version of kanata does not handle `defsrc` consistently compared to other versions and other operating systems. This variant will be of interest to you for any of the following reasons:
- you are a new user
- you are a cross-platform user
- you use multiple language layouts within Windows and want kanata to handle the key positions consistently

This variant contains the same output change as in the `scancode` variant below, and also changes the input to also operate on scancodes.

---

**NOTE:** The `kanata_legacy_output.exe` variant has the same input `defsrc` handling as the standard `kanata.exe` file. It uses the same output mechanism as the standard `kanata.exe` variant in version 1.6.1 and earlier. In other words the formerly `experimental_scancode` variant is now the default binary. The non-legacy variants contain changes for [an issue](https://github.com/jtroo/kanata/issues/567); the fix is omitted from this legacy variant. The legacy variant is included in case issues are found with the new output mechanism.

---

</details>

## Linux

<details>
<summary>Instructions</summary>

**NOTE:** All Linux binaries are compiled for x86 architectures only.

Download `kanata`.

Run it in a terminal and point it to a valid configuration file. Kanata does not start a background process, so the window needs to stay open after startup. See [this discussion](https://github.com/jtroo/kanata/discussions/130) for how to set up kanata with systemd.
```
chmod +x kanata   # may be downloaded without executable permissions
sudo ./kanata --cfg <cfg_file>`
```

To avoid requiring `sudo`, [follow the instructions here](https://github.com/jtroo/kanata/wiki/Avoid-using-sudo-on-Linux).

</details>

## macOS

<details>
<summary>Instructions</summary>

**WARNING**: feature support on macOS [is limited](https://github.com/jtroo/kanata/blob/main/docs/platform-known-issues.adoc#macos).

### Install Karabiner driver for macOS 11 and newer:
- Install the [V5 Karabiner VirtualHiDDevice Driver](https://github.com/pqrs-org/Karabiner-DriverKit-VirtualHIDDevice/blob/main/dist/Karabiner-DriverKit-VirtualHIDDevice-5.0.0.pkg).

To activate it:

```
sudo /Applications/.Karabiner-VirtualHIDDevice-Manager.app/Contents/MacOS/Karabiner-VirtualHIDDevice-Manager activate
```

Then you need to run the daemon. You should run this in the background somehow or leave the terminal window where you run this command open.

```
sudo '/Library/Application Support/org.pqrs/Karabiner-DriverKit-VirtualHIDDevice/Applications/Karabiner-VirtualHIDDevice-Daemon.app/Contents/MacOS/Karabiner-VirtualHIDDevice-Daemon'
```

### Install Karabiner driver for macOS 10 and older:

- Install the [Karabiner kernel extension](https://github.com/pqrs-org/Karabiner-VirtualHIDDevice).

### After installing the appropriate driver for your OS

Download a `kanata_macos` variant.

Run it in a terminal and point it to a valid configuration file. Kanata does not start a background process, so the window needs to stay open after startup.

Example
```
chmod +x kanata_macos_arm64   # may be downloaded without executable permissions
sudo ./kanata_macos_arm64 --cfg <cfg_file>`
```

</details>

## cmd\_allowed variants

<details>
<summary>Explanation</summary>

The binaries with the name `cmd_allowed` are conditionally compiled with the `cmd` action enabled.

Using the regular binaries, there is no way to get the `cmd` action to work. This action is restricted behind conditional compilation because I consider the action to be a security risk that should be explicitly opted into and completely forbidden by default.

</details>

## wintercept variants

<details>
<summary>Explanation and instructions</summary>

### Warning: known issue

This issue in the Interception driver exists: https://github.com/oblitum/Interception/issues/25. This will affect you if you put your PC to sleep instead of shutting it down, or if you frequently plug/unplug USB devices.

### Description

These variants use the [Interception driver](https://github.com/oblitum/Interception) instead of Windows hooks. You will need to install the driver using the release or from the [copy in this repo](https://github.com/jtroo/kanata/tree/main/assets). The benefit of using this driver is that it is a lower-level mechanism than Windows hooks. This means `kanata` will work in more applications.

### Steps to install the driver

- extract the `.zip`
- run a shell with administrator privilege
- run the script `"command line installer/install-interception.exe"`
- reboot

### Additional installation steps

The above steps are those recommended by the interception driver author. However, I have found that those steps work inconsistently and sometimes the dll stops being able to be loaded. I think it has something to do with being installed in the privileged location of `system32\drivers`.

To help with the dll issue, you can copy the following file in the zip archive to the directory that kanata starts from: `Interception\library\x64\interception.dll`.

E.g. if you start kanata from your `Documents` folder, put the file there:

```
C:\Users\my_user\Documents\
    kanata_wintercept.exe
    kanata.kbd
    interception.dll
```

</details>

## kanata\_passthru.dll

<details>
<summary>Explanation and instructions</summary>

The Windows `kanata_passthru.dll` file allows using Kanata as a library within AutoHotkey to avoid conflicts between keyboard hooks installed by both. You can channel keyboard input events received by AutoHotkey into Kanata's keyboard engine and get the transformed keyboard output events (per your Kanata config) that AutoHotkey can then send to the OS.

To make use of this, download `kanata_passthru.dll`, then the [simulated_passthru_ahk](https://github.com/jtroo/kanata/blob/main/docs/simulated_passthru_ahk) folder with a brief example, place the dll there, open `kanata_passthru.ahk` to read what the example does and then double-click to launch it.
</details>

## sha256 checksums

<details>
<summary>Sums</summary>

```
TODO: fill this out
```

</details>
