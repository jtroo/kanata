Create a user for ktrl and add it to the input groups

```
sudo useradd -r -s /bin/false ktrl
sudo groupadd uinput
sudo usermod -aG input ktrl
sudo usermod -aG uinput ktrl

# If you're using the sound feature
sudo usermod -aG audio ktrl
```

Add a new udev rule

```
sudo touch /etc/udev/rules.d/99-uinput.rules
```

Add the following line to it

```
KERNEL=="uinput", MODE="0660", GROUP="uinput", OPTIONS+="static_node=uinput"
```

If you are using the default `/opt/ktrl` directory - 

```
sudo chown -R ktrl:$USER /opt/ktrl
sudo chmod -R 0770 /opt/ktrl
```
