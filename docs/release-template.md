## Changelog (since <previous_version_here>)

- <fill this out>

## Sample configuration file

The attached `kanata.kbd` file is tested to work with the current version. The one in the `main` branch of the repository may have extra features that are not supported in this release.

## Windows

Download `kanata.exe`. Optionally, download `kanata.kbd`. With the two files in the same directory, you can double-click the `exe` to start kanata.

You need to run `kanata.exe` via `cmd` or `powershell` to use a different configuration file:

`kanata.exe --cfg <cfg_file>`

You can also set up a [toolbar shortcut](https://github.com/jtroo/kanata/wiki/Toolbar-shortcut-for-Windows-10).

## Linux

Download `kanata`.

Run it in a terminal and point it to a valid configuration file:

```
chmod +x kanata   # may be downloaded without executable permissions
sudo ./kanata --cfg <cfg_file>`
```

To avoid requiring `sudo`, [follow the instructions here](https://github.com/jtroo/kanata/wiki/Avoid-using-sudo-on-Linux).

## cmd_allowed variants

The binaries with the name `cmd_allowed` are conditionally compiled with the `cmd` action enabled.

Using the regular binaries, there is no way to get the `cmd` action to work. This action is restricted behind conditional compilation because I consider the action to be a security risk that should be explicitly opted into and completely forbidden by default.

## wintercept variants

### Warning: known issue

This issue in the Interception driver exists: https://github.com/oblitum/Interception/issues/25. This will affect you if you put your PC to sleep instead of shutting it down, or if you frequently plug/unplug USB devices.

### Description

These variants use the [Interception driver](http://www.oblita.com/interception) instead of Windows hooks. You will need to install the driver using the assets from the linked website or from the [copy in this repo](https://github.com/jtroo/kanata/tree/main/assets). The benefit of using this driver is that it is a lower-level mechanism than Windows hooks. This means `kanata` will work in more applications, including administrator-privileged apps.

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

## sha256 checksums

```
<fill this out>
```
