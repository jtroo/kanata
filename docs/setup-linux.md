# Instructions

In Linux, kanata needs to be able to access the input and uinput subsystem to inject events. To do this, your user needs to have permissions. Follow the steps in this page to obtain user permissions.

### 1. If the uinput group does not exist, create a new group

```bash
sudo groupadd uinput
```

### 2. Add your user to the input and the uinput group

```bash
sudo usermod -aG input $USER
sudo usermod -aG uinput $USER
```

Make sure that it's effective by running `groups`. You might have to logout and login.

### 3. Make sure the uinput device file has the right permissions.

#### Create a new file:
`/etc/udev/rules.d/99-input.rules`

#### Insert the following in the code
```bash
KERNEL=="uinput", MODE="0660", GROUP="uinput", OPTIONS+="static_node=uinput"
```

#### Machine reboot or run this to reload
```bash
sudo udevadm control --reload-rules && sudo udevadm trigger
```

#### Verify settings by following command:
```bash
ls -l /dev/uinput
```

#### Output:
```bash
crw-rw---- 1 root date uinput /dev/uinput
```

### 4. Make sure the uinput drivers are loaded

You may need to run this command whenever you start kanata for the first time:

```
sudo modprobe uinput
```
### 5a. To create and enable a systemd daemon service

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
ExecStart=/usr/bin/sh -c 'exec $$(which kanata) --cfg $${HOME}/.config/kanata/config.kbd'
Restart=no

[Install]
WantedBy=default.target
```

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
