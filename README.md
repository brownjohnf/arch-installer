# Arch Installer

This script can automate the install process for Arch Linux. It's forked from
https://github.com/mdaffin/arch-pkgs/blob/master/installer/install-arch at
commit
https://github.com/mdaffin/arch-pkgs/commit/f8da560b8af13ba29a409afe048ea921a32b3b66.

## Display Settings

If you are installing in certain systems, such as a Dell XPS laptop, you
need to set custom graphics boot options. In the case of the XPS, it
should be `nouveau.modeset=0`.

You can do this by pressing `e` at the EFI boot screen. After editing
the startup command, press `ENTER` (or sometimes `Ctl-x`) to boot the
system. Once the
installation is complete, you'll need to modify the options to set it
permanently.

## Usage

You can access this script at a shortened URL: https://git.io/fpl2Y. This allows
you to use it without copy/pasting:

```sh
# curl -sL https://git.io/fpl2Y | bash
```

### Internet Connection, NetworkManager

If installing from a wifi device, start NetworkManager, and then connect
to the wifi network:

```
# systemctl start NetworkManager
# nmcli radio
WIFI-HW  WIFI     WWAN-HW  WWAN
enabled  enabled  enabled  enabled

# nmcli device
DEVICE  TYPE      STATE         CONNECTION
wlan0   wifi      disconnected  --
eth0    ethernet  unavailable   --
lo      loopback  unmanaged     --
```

Then to actually connect to a wireless AP:

```
# nmcli device wifi rescan
# nmcli device wifi list
# nmcli device wifi connect <ssid> password <password>
```

### Network Connection, netctl

If using a vanilla archiso, NetworkManager won't be installed.

``` sh
# Interactively select a wifi network and connect. This can take some time;
# just be patient and it will work.
wifi-menu
```

```

```

### Options

* `CLEAN=1`: Unmount any mounted partitions, volume groups, crypt mounts, etc.
* `ZFS=1`: Install on encrypted ZFS instead of LVM

You'll be prompted for things like encryption passwords, etc.
