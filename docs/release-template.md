# Changelog

- <fill this out>

# Sample configuration file

The attached `kanata.kbd` file is tested to work with the current version. The one in the `main` branch of the repository may have extra features that are not supported in this release.

# Windows

Download `kanata.exe`. Optionally, download `kanata.kbd`. With the two files in the same directory, you can double-click the `exe` to start kanata.

You need to run `kanata.exe` via `cmd` or `powershell` to use a different configuration file:

`kanata.exe --cfg <cfg_file>`

You can also set up a [toolbar shortcut](https://github.com/jtroo/kanata/wiki/Toolbar-shortcut-for-Windows-10).

# Linux

Download `kanata`.

Run it in a terminal and point it to a valid configuration file:

```
chmod +x kanata   # may be downloaded without executable permissions
sudo ./kanata --cfg <cfg_file>`
```

To avoid requiring `sudo`, [follow the instructions here](https://github.com/kmonad/kmonad/blob/master/doc/faq.md#linux).

# cmd_allowed variants

The binaries `kanata_cmd_allowed` and `kanata_cmd_allowed.exe` are conditionally compiled with the `cmd` action enabled.

Using the regular binaries, there is no way to get the `cmd` action to work. This action is restricted behind conditional compilation because I consider the action to be a security risk that should be explicitly opted into and completely forbidden by default.

# sha256 checksums

```
< fill this out>
```
