# Instructions

On macOS, kanata grabs the keyboard via the
[Karabiner-DriverKit-VirtualHIDDevice](https://github.com/pqrs-org/Karabiner-DriverKit-VirtualHIDDevice)
driver. The kanata process must run as root because the Karabiner virtual HID
daemon exposes its IPC under `/Library/Application Support/org.pqrs/tmp/rootonly/`,
which only root can access. This page walks through installing the driver,
granting permissions, and (optionally) registering kanata as a LaunchDaemon so
it starts at boot.

### 1. Prerequisites

- macOS 11 (Big Sur) or newer.
- Xcode Command Line Tools, only if you intend to build kanata from source:

```sh
xcode-select --install
```

### 2. Install Karabiner-DriverKit-VirtualHIDDevice

The supported driver version is `v6.2.0`. Kanata's bundled
`karabiner-driverkit` crate is built against that release's IPC, and pqrs
ships protocol changes between minor versions, so newer driver releases
are not guaranteed to work. Download the `.pkg` from the
[v6.2.0 release page](https://github.com/pqrs-org/Karabiner-DriverKit-VirtualHIDDevice/releases/tag/v6.2.0)
and run the installer.

Then activate the driver and approve its system extension:

```sh
sudo /Applications/.Karabiner-VirtualHIDDevice-Manager.app/Contents/MacOS/Karabiner-VirtualHIDDevice-Manager forceActivate
```

Open `System Settings > General > Login Items & Extensions > Driver Extensions`
and toggle on the entry for `org.pqrs.Karabiner-DriverKit-VirtualHIDDevice`.
A reboot may be required if you previously ran `deactivate`.

Verify the daemon is running:

```sh
sudo launchctl list | grep org.pqrs
```

You should see `org.pqrs.service.daemon.Karabiner-VirtualHIDDevice-Daemon`
listed.

### 3. Install the kanata binary

Either download a pre-built binary from the
[releases page](https://github.com/jtroo/kanata/releases) and place it on
your `PATH`:

```sh
chmod +x kanata-macos-arm64
sudo mv kanata-macos-arm64 /usr/local/bin/kanata
```

Or build from source:

```sh
git clone https://github.com/jtroo/kanata && cd kanata
cargo build --release
sudo cp target/release/kanata /usr/local/bin/kanata
```

### 4. Grant Input Monitoring permission

kanata needs Input Monitoring permission in
`System Settings > Privacy & Security > Input Monitoring`. The first time you
run kanata as root, macOS will prompt you to add the binary; you can also
pre-add `/usr/local/bin/kanata` (or wherever you installed it) by clicking the
`+` button and selecting the binary.

Mouse-button input additionally needs Accessibility or Input Monitoring
permission for the same binary.

### 5. Smoke test from terminal

Pick a sample config (or your own) and run:

```sh
sudo kanata -c cfg_samples/simple.kbd
```

You should see log lines similar to:

```
[INFO] kanata v1.x.x starting
[INFO] entering the processing loop
[INFO] init: catching only releases and sending immediately
[INFO] Sleeping for 2s. Please release any keys now.
[INFO] Starting kanata proper
```

Press a remapped key to confirm the mapping fires. `ctrl+space+esc` (held
together) cleanly exits kanata.

If kanata aborts immediately with a `libc++abi` / `filesystem_error` message,
the on-process diagnostic will print three likely causes: not running as root,
the Karabiner driver not installed/approved, or another process is grabbing
the keyboard exclusively. See the troubleshooting section below.

### 6. (Optional) Install as a LaunchDaemon

For login-time / boot-time startup, use the sample LaunchDaemon plist in
[`cfg_samples/kanata.plist`](../cfg_samples/kanata.plist).

Edit the two paths in `ProgramArguments` to point at your kanata binary and
your config file (the defaults are `/usr/local/bin/kanata` and
`/etc/kanata/kanata.kbd`), then install:

```sh
sudo cp cfg_samples/kanata.plist /Library/LaunchDaemons/dev.kanata.kanata.plist
sudo chown root:wheel /Library/LaunchDaemons/dev.kanata.kanata.plist
sudo launchctl bootstrap system /Library/LaunchDaemons/dev.kanata.kanata.plist
```

Verify it is running:

```sh
sudo launchctl print system/dev.kanata.kanata
```

Logs are written to `/var/log/kanata.log`.

After editing your kanata config (or the plist itself), reload with:

```sh
sudo launchctl kickstart -k system/dev.kanata.kanata
```

### 7. Uninstall

Remove the LaunchDaemon (if you installed it):

```sh
sudo launchctl bootout system/dev.kanata.kanata
sudo rm /Library/LaunchDaemons/dev.kanata.kanata.plist
```

Remove the kanata binary:

```sh
sudo rm /usr/local/bin/kanata
```

If you are also fully removing the Karabiner driver:

```sh
sudo /Applications/.Karabiner-VirtualHIDDevice-Manager.app/Contents/MacOS/Karabiner-VirtualHIDDevice-Manager deactivate
```

You may then delete the `Karabiner-VirtualHIDDevice-Manager.app` from
`/Applications/`.

### 8. Troubleshooting

- **`libc++abi: terminating due to uncaught exception ... filesystem_error`**
  on startup: the three likely causes, in order, are (1) kanata is not running
  as root, (2) the Karabiner driver is not installed or its system extension
  is not approved, (3) another process is already grabbing the keyboard
  exclusively. The kanata process prints this same hint to stderr right before
  it aborts.
- **A remapped key fires but nothing types**: confirm the kanata binary has
  Input Monitoring permission in `System Settings > Privacy & Security`. If
  you reinstalled the binary in place, macOS sometimes invalidates the
  permission, so toggle it off and back on.
- **Stuck modifier or layer after the lock screen / fast user switch**:
  kanata pauses its grab while the screen is locked or another user has the
  console; the first keystroke after unlock can be dropped. This is by design.
- See [`docs/platform-known-issues.adoc`](./platform-known-issues.adoc) for
  the full list of known macOS issues.
