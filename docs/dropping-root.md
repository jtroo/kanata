Create a user for kanata and add it to the input groups

```
sudo useradd -r -s /bin/false kanata
sudo groupadd uinput
sudo usermod -aG input kanata
sudo usermod -aG uinput kanata
```

Add a new udev rule

```
sudo touch /etc/udev/rules.d/99-uinput.rules
```

Add the following line to it

```
KERNEL=="uinput", MODE="0660", GROUP="uinput", OPTIONS+="static_node=uinput"
```

If you are using the default `/opt/kanata` directory -

```
sudo chown -R kanata:$USER /opt/kanata
sudo chmod -R 0770 /opt/kanata
```
