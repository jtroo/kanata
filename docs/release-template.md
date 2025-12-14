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

Download the appropriate `kanata-windows-variant.zip` file for your machine CPU. Extract and move the desired binary variant to its intended location. Optionally, download `kanata.kbd`. With the two files in the same directory, you can double-click the extracted `.exe` file to start kanata. Kanata does not start a background process, so the window needs to stay open after startup. See [this discussion](https://github.com/jtroo/kanata/discussions/193) for tips to run kanata in the background.

You need to run via `cmd` or `powershell` to use a different configuration file:

`kanata_windows_binaryvariant.exe --cfg <cfg_file>`

### Binary variants

Explanation of items in the binary variant:

- x64 vs. arm64:
  - Select x64 if your machine's CPU is Intel or AMD. If ARM, use arm64.
- tty vs gui:
  - tty runs in a terminal, gui runs as a system tray application
- cmd\_allowed vs. not
  - cmd\_allowed allows the `cmd` actions; otherwise, they are compiled out of the application
- winIOv2 vs. wintercept
  - winIOv2 uses the LLHOOK and SendInput Windows mechanisms to intercept and send events.
  - wintercept uses the [Interception driver](https://github.com/oblitum/Interception). Beware of its known issue that disables keyboards and mice until system reboot: [Link to issue](https://github.com/oblitum/Interception/issues/25).
    - you will need to install the driver using the release or from the [copy in this repo](https://github.com/jtroo/kanata/tree/main/assets).
    - the benefit of using this driver is that it is a lower-level mechanism than Windows hooks, and `kanata` will work in more applications.

### wintercept installation

#### Steps to install the driver

- extract the `.zip`
- run a shell with administrator privilege
- run the script `"command line installer/install-interception.exe"`
- reboot

#### Additional installation steps

The above steps are those recommended by the interception driver author. However, I have found that those steps work inconsistently and sometimes the dll stops being able to be loaded. I suspect it has something to do with being installed in the privileged location of `system32\drivers`.

To help with the dll issue, you can copy the following file in the zip archive to the directory that kanata starts from: `Interception\library\x64\interception.dll`.

E.g. if you start kanata from your `Documents` folder, put the file there:

**Example:**

```
C:\Users\my_user\Documents\
    kanata_windows_wintercept_x64.exe
    kanata.kbd
    interception.dll
```

### kanata\_passthru_x64.dll

The Windows `kanata_passthru_x64.dll` file allows using Kanata as a library within AutoHotkey to avoid conflicts between keyboard hooks installed by both. You can channel keyboard input events received by AutoHotkey into Kanata's keyboard engine and get the transformed keyboard output events (per your Kanata config) that AutoHotkey can then send to the OS.

To make use of this, take `kanata_passthru_x64.dll`, then the [simulated\_passthru\_ahk](https://github.com/jtroo/kanata/blob/main/docs/simulated_passthru_ahk) folder with a brief example, place the dll there, open `kanata_passthru.ahk` to read what the example does and then double-click to launch it.

</details>

## Linux

<details>
<summary>Instructions</summary>

Download the `kanata-linux-x64.zip` file.

 Extract and move the desired binary variant to its intended location. Run the binary in a terminal and point it to a valid configuration file. Kanata does not start a background process, so the window needs to stay open after startup. See [this discussion](https://github.com/jtroo/kanata/discussions/130) for how to set up kanata with systemd.

**Example:**

```
chmod +x kanata   # may be downloaded without executable permissions
sudo ./kanata_linux_x64 --cfg <cfg_file>`
```

To avoid requiring `sudo`, [follow the instructions here](https://github.com/jtroo/kanata/wiki/Avoid-using-sudo-on-Linux).

### Binary variants

Explanation of items in the binary variant:

- cmd\_allowed vs. not
  - cmd\_allowed allows the `cmd` actions; otherwise, they are compiled out of the application

</details>

## macOS

<details>
<summary>Instructions</summary>

The supported Karabiner driver version in this release is `v6.2.0`.

**WARNING**: macOS does not support mouse as input. The `mbck` and `mfwd` mouse button actions are also not operational.

### Binary variants

Explanation of items in the binary variant:

- x64 vs. arm64:
  - Select x64 if your machine's CPU is Intel. If ARM, use arm64.
- cmd\_allowed vs. not
  - cmd\_allowed allows the `cmd` actions; otherwise, they are compiled out of the application

### Instructions for macOS 11 and newer

You must use the Karabiner driver version `v6.2.0`.

Please read through this issue comment:

https://github.com/jtroo/kanata/issues/1264#issuecomment-2763085239

Also have a read through this discussion:

https://github.com/jtroo/kanata/discussions/1537

At some point it may be beneficial to provide concise and accurate instructions within this documentation. The maintainer (jtroo) does not own macOS devices to validate; please contribute the instructions to the file `docs/release-template.md` if you are able.

### Install Karabiner driver for macOS 10 and older:

- Install the [Karabiner kernel extension](https://github.com/pqrs-org/Karabiner-VirtualHIDDevice).

### After installing the appropriate driver for your OS (both macOS <=10 and >=11)

Download the appropriate `kanata-macos-variant.zip` for your machine CPU.

Extract and move the desired binary variant to its intended location. Run the binary in a terminal and point it to a valid configuration file. Kanata does not start a background process, so the window needs to stay open after startup.

**Example:**

```
chmod +x kanata_macos_arm64   # may be downloaded without executable permissions
sudo ./kanata_macos_arm64 --cfg <cfg_file>`
```

### Add permissions

If Kanata is not behaving correctly, you may need to add permissions. Please see this issue: [link to macOS permissions issue](https://github.com/jtroo/kanata/issues/1211).

</details>

## sha256 checksums

<details>
<summary>Sums</summary>

```
TODO: fill this out
```

</details>
