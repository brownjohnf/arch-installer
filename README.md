# Arch Installer

This script can automate the install process for Arch Linux. It's forked from
https://github.com/mdaffin/arch-pkgs/blob/master/installer/install-arch at
commit
https://github.com/mdaffin/arch-pkgs/commit/f8da560b8af13ba29a409afe048ea921a32b3b66.

## Usage

You can access this script at a shortened URL: https://git.io/fpl2Y. This allows
you to use it without copy/pasting:

```sh
# curl -sL https://git.io/fpl2Y | bash
```

### Internet Connection

If installing from a wifi device:

nmcli radio
WIFI-HW  WIFI     WWAN-HW  WWAN
enabled  enabled  enabled  enabled
# nmcli device
DEVICE  TYPE      STATE         CONNECTION
wlan0   wifi      disconnected  --
eth0    ethernet  unavailable   --
lo      loopback  unmanaged     --

Then to actually connect to a wireless AP:

# nmcli device wifi rescan
# nmcli device wifi list
# nmcli device wifi connect SSID-Name password wireless-password

