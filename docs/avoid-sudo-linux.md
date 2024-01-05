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

#### Create a new file name:
`/etc/udev/rules.d/99-input.rules`

#### Insert the following in the code
```bash
KERNEL=="uinput", MODE="0660", GROUP="uinput", OPTIONS+="static_node=uinput"
```

#### Machine reboot or run this to reload
```bash
sudo sh -c 'udevadm control –reload; udevadm trigger -v –name-match uinput'
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

# Credits

The original text was taken and adapted from: https://github.com/kmonad/kmonad/blob/master/doc/faq.md#linux
