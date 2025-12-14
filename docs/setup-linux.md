# Instructions

In Linux, kanata needs to be able to access the input and uinput subsystem to inject events. To do this, your user needs to have permissions. Follow the steps in this page to obtain user permissions.

### 1. Create the uinput group (if it doesn’t exist)

```bash
sudo groupdel uinput 2>/dev/null
sudo groupadd --system uinput
```

### 2. Add your user to the `input` and `uinput` group

```sh
sudo usermod -aG input $USER
sudo usermod -aG uinput $USER
```

Verify:

```sh
groups
```

You may need to log out and back in for it to take effect.

### 3. Load the uinput kernel module

```sh
sudo modprobe uinput
```

This ensures /dev/uinput exists.

### 4. Make sure the uinput device file has the right permissions.

Create the udev rule:

```bash
sudo tee /etc/udev/rules.d/99-input.rules > /dev/null <<EOF
KERNEL=="uinput", MODE="0660", GROUP="uinput", OPTIONS+="static_node=uinput"
EOF
```

Reload udev rules:

#### Machine reboot or run this to reload

```bash
sudo udevadm control --reload-rules && sudo udevadm trigger
```

Verify:

```bash
ls -l /dev/uinput
```

Expected output:

```bash
crw-rw---- 1 root uinput 10, <minor> <MMM DD HH:MM> /dev/uinput
```

## 5. Run Kanata immediately if the group change isn’t active

If `uinput` is not listed in `groups` even after adding your user:

```bash
newgrp uinput -c kanata
```

This temporarily gives the current shell the `uinput` group so kanata can access `/dev/uinput` until the next login.

```bash
newgrp uinput -c kanata
```

### 6. To create and enable a systemd daemon service

Run this command first:

```bash
mkdir -p ~/.config/systemd/user
```

Then add this to: `~/.config/systemd/user/kanata.service`:

```bash
[Unit]
Description=Kanata keyboard remapper
Documentation=https://github.com/jtroo/kanata

[Service]
Environment=PATH=/usr/local/bin:/usr/local/sbin:/usr/bin:/bin
#   Uncomment the 4 lines beneath this to increase process priority
#   of Kanata in case you encounter lagginess when resource constrained.
#   WARNING: doing so will require the service to run as an elevated user such as root.
#   Implementing least privilege access is an exercise left to the reader.
#
# CPUSchedulingPolicy=rr
# CPUSchedulingPriority=99
# IOSchedulingClass=realtime
# Nice=-20
Type=simple
ExecStart=/usr/bin/sh -c 'exec $$(which kanata) --cfg $${HOME}/.config/kanata/config.kbd --no-wait'
Restart=on-failure
RestartSec=3

[Install]
WantedBy=default.target
```

Note: The `--no-wait` flag is required for `Restart=on-failure` to work.
Without it, kanata waits for user input on exit, which blocks automatic restart.

Make sure to update the executable location for sh in the snippet above.
This would be the line starting with `ExecStart=/usr/bin/sh -c`.
You can check the executable path with:

```bash
which sh
```

Also, verify if the path to kanata is included in the line `Environment=PATH=[...]`.
For example, if executing `which kanata` returns `/home/[user]/.cargo/bin/kanata`, the `PATH` line should be appended with `/home/[user]/.cargo/bin` or `:%h/.cargo/bin`.
`%h` is one of the specifiers allowed in systemd, more can be found in https://www.freedesktop.org/software/systemd/man/latest/systemd.unit.html#Specifiers

Then run:

```bash
systemctl --user daemon-reload
systemctl --user enable kanata.service
systemctl --user start kanata.service
systemctl --user status kanata.service   # check whether the service is running
```

### 5b. To create and enable an OpenRC daemon service

Edit new file `/etc/init.d/kanata` as root, replacing \<username\> as appropriate:

```bash
#!/sbin/openrc-run

command="/home/<username>/.cargo/bin/kanata"
#command_args="--config=/home/<username>/.config/kanata/kanata.kbd"

command_background=true
pidfile="/run/${RC_SVCNAME}.pid"

command_user="<username>"
```

Then run:

```
sudo chmod +x /etc/init.d/kanata # script must be executable
sudo rc-service kanata start
rc-status # check that kanata isn't listed as [ crashed ]
sudo rc-update add kanata default # start the service automatically at boot
```

# Credits

The original text was taken and adapted from: https://github.com/kmonad/kmonad/blob/master/doc/faq.md#linux
