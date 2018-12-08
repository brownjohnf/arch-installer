#!/bin/bash

# WARNING: this script will destroy data on the selected disk.

# To use ZFS, you'll need to press 'e' at the live USB bootscreen and add
# (anywhere in the line) 'cow_spacesize=1G'. This is copy-on-write storage in
# RAM, so you can adjust as necessary, but you need to not allocate more than
# you have available in memory.

set -xeuo pipefail
trap 's=$?; echo "$0: Error on line "$LINENO": $BASH_COMMAND"; exit $s' ERR

CLEAN=${CLEAN:-''}
ZFS=${ZFS:-''}

# If CLEAN is set, clean up any broken state we may have lying around
if [ -n $CLEAN ]; then
  umount /mnt/boot || true
  umount /mnt/var || true
  umount /mnt/home || true
  umount /mnt/data || true
  umount /mnt || true

  vgchange -a n lvmroot || true
  pvremove -ff /dev/mapper/cryptroot || true

  zpool destroy zroot || true

  cryptsetup close /dev/mapper/cryptroot || true
fi

# Ensure we've got the system booted in EFI
ls /sys/firmware/efi/efivars > /dev/null

MIRRORLIST_URL="https://www.archlinux.org/mirrorlist/?country=US&protocol=https&use_mirror_status=on"

# TODO figure out how to make this work without a custom archiso image
# if [ -n $ZFS ]; then
# cat <<EOF >>/etc/pacman.conf
# [archzfs]
# Server = http://archzfs.com/\$repo/x86_64
# EOF
#
#   pacman-key --recv-keys F75D9D76
#   pacman-key --lsign-key F75D9D76
#
#   pacman -Sy --needed --noconfirm archzfs-archiso-linux
# else
# fi

pacman -Sy --needed --noconfirm pacman-contrib dmidecode

# Only rank the mirrors if we're not cleaning up, indicating this is a fresh run
if [ -z $CLEAN ]; then
  echo "Updating mirror list"
  curl -s "$MIRRORLIST_URL" | \
      sed -e 's/^#Server/Server/' -e '/^#/d' | \
      rankmirrors -n 5 - > /etc/pacman.d/mirrorlist
fi

### Get infomation from user ###
devicelist=$(lsblk -dplnx size -o name,size | grep -Ev "boot|rpmb|loop" | tac)
device=$(dialog --stdout --menu "Select installtion disk" 0 0 0 ${devicelist}) || exit 1
clear

luks_password=$(dialog --stdout --inputbox "Enter LUKS password (will print)" 0 0) || exit 1
clear
: ${luks_password:?"luks_password cannot be empty"}

hostname=$(dialog --stdout --inputbox "Enter hostname" 0 0) || exit 1
clear
: ${hostname:?"hostname cannot be empty"}

wifi=false
if dialog --yesno "Configure wifi for target system?" 0 0; then
  wifi=true
fi
clear

if [ "$wifi" == "true" ]; then
  wifi_ssid=$(dialog --stdout --inputbox "Enter wifi SSID" 0 0) || exit 1
  clear
  : ${wifi_ssid:?"Wifi SSID cannot be empty"}

  wifi_password=$(dialog --stdout --inputbox "Enter wifi password (will print)" 0 0) || exit 1
  clear
  : ${wifi_password:?"Wifi password cannot be empty"}
fi

user=$(dialog --stdout --inputbox "Enter admin username" 0 0) || exit 1
clear
: ${user:?"user cannot be empty"}

password=$(dialog --stdout --inputbox "Enter admin password (will print)" 0 0) || exit 1
clear
: ${password:?"password cannot be empty"}

### Set up logging ###
exec 1> >(tee "stdout.log")
exec 2> >(tee "stderr.log")

timedatectl set-ntp true

### Setup the disk and partitions ###
parted --script "${device}" -- \
  mklabel gpt \
  mkpart ESP fat32 1Mib 129MiB \
  set 1 boot on \
  mkpart primary ext4 129Mib 1024MiB \
  mkpart primary ext4 1024MiB 100%

# Simple globbing was not enough as on one device I needed to match /dev/mmcblk0p1
# but not /dev/mmcblk0boot1 while being able to match /dev/sda1 on other devices.
part_boot="$(ls ${device}* | grep -E "^${device}p?1$")"
part_root="$(ls ${device}* | grep -E "^${device}p?3$")"

# Wipe any old fs stuff from the new partitions
wipefs "${part_boot}"
wipefs "${part_root}"
dd if=/dev/urandom of=$part_root bs=512 count=20480

# Set up the EFI partition
mkfs.vfat -F32 "${part_boot}"

cryptsetup -v luksFormat --type luks2 $part_root <<< "${luks_password}"
cryptsetup open $part_root cryptroot <<< "${luks_password}"

# If we're doing ZFS, we set up ZFS partitioning
if [ -n $ZFS ]; then
  modprobe zfs
  zpool create -f zroot /dev/disk/by-id/dm-name-cryptroot
  zfs create -o atime=off -o compression=on -o mountpoint=none zroot/ROOT

  zfs create -o mountpoint=/ zroot/ROOT/default || true

  for path in /home /tmp /var /var/log /var/log/journal /etc /data /data /docker; do
    zfs create -o mountpoint=$path zroot/ROOT$path || true
  done
  zfs unmount -a

  # Set up stuff for the /tmp dataset
  zfs set sync=disabled zroot/ROOT/tmp
  zfs set setuid=off zroot/ROOT/tmp
  zfs set devices=off zroot/ROOT/tmp
  zfs set compression=off zroot/ROOT/tmp

  # Set up posix ACL for the journal
  zfs set acltype=posixacl zroot/ROOT/var/log/journal
  zfs set xattr=sa zroot/ROOT/var/log/journal

  echo "zroot/ROOT/default / zfs defaults,noatime 0 0" >> /etc/fstab
  zpool set bootfs=zroot/ROOT/default zroot
  zpool export zroot
  zpool import -d /dev/disk/by-id -R /mnt zroot
  zpool set cachefile=/etc/zfs/zpool.cache zroot
  mkdir -p /mnt/etc/zfs
  cp /etc/zfs/zpool.cache /mnt/etc/zfs/zpool.cache
  mkdir -p /mnt/boot
  mount $part_boot /mnt/boot
else
  # If we're doing LVM, then we'll set up LVM partitions
  pvcreate /dev/mapper/cryptroot
  vgcreate lvmroot /dev/mapper/cryptroot
  # If the root partition is less than 10G, make a 3G root partition
  if [ $(lsblk -o SIZE --noheadings --nodeps -b $part_root) -lt 10000000000 ]; then
    lvcreate -L 3GB lvmroot -n root
  else
    lvcreate -l 15%VG lvmroot -n root
  fi
  lvcreate -l 10%VG lvmroot -n var
  lvcreate -l 20%VG lvmroot -n home
  lvcreate -l 100%FREE lvmroot -n data

  mkfs.ext4 /dev/lvmroot/root
  mkfs.ext4 /dev/lvmroot/var
  mkfs.ext4 /dev/lvmroot/home
  mkfs.ext4 /dev/lvmroot/data

  function mount_all () {
    sleep 1
    mount /dev/lvmroot/root /mnt

    mkdir -p /mnt/{boot,var,home,data}

    mount "${part_boot}" /mnt/boot
    mount /dev/lvmroot/var /mnt/var
    mount /dev/lvmroot/home /mnt/home
    mount /dev/lvmroot/data /mnt/data
  }
  mount_all

  # Unmount, lock, unlock and remount the partition to ensure it works
  umount /mnt/boot
  umount /mnt/var
  umount /mnt/home
  umount /mnt/data
  umount /mnt

  # Deactivate the volume group. If you don't do this, you can't close the
  # cryptroot
  vgchange -a n lvmroot

  cryptsetup close cryptroot
  cryptsetup open $part_root cryptroot <<< "${luks_password}"

  mount_all
fi

### Install and configure the basic system ###

#usage: pacstrap [options] root [packages...]
#
#  Options:
#    -C config      Use an alternate config file for pacman
#    -c             Use the package cache on the host, rather than the target
#    -G             Avoid copying the host's pacman keyring to the target
#    -i             Prompt for package confirmation when needed
#    -M             Avoid copying the host's mirrorlist to the target
#
#    -h             Print this help message
#
#pacstrap installs packages to the specified new root directory. If no packages
#are given, pacstrap defaults to the "base" group.
pacstrap /mnt \
  base \
  vim \
  networkmanager \
  openssh \
  sudo
genfstab -t PARTUUID /mnt >> /mnt/etc/fstab

# If running ZFS, edit the fstab and remove the zfs fs
if [ -n $ZFS ]; then sed -i '/zfs/d' /mnt/etc/fstab; fi

# Set up the hostname and /etc/hosts
echo $hostname > /mnt/etc/hostname
hostname_short=$(echo $hostname | cut -d '.' -f 1)
cat <<EOF > /mnt/etc/hosts
127.0.0.1 localhost
::1       localhost
::1       $hostname $hostname_short
127.0.1.1	$hostname $hostname_short
EOF

if [ -n $ZFS ]; then
cat <<EOF >>/mnt/etc/pacman.conf
[archzfs]
Server = http://archzfs.com/\$repo/x86_64
EOF

  arch-chroot /mnt pacman-key --recv-keys F75D9D76
  arch-chroot /mnt pacman-key --lsign-key F75D9D76

  arch-chroot /mnt pacman -Sy --needed --noconfirm zfs-linux
fi

# Set up the hooks correctly for allowing us to unlock the encrypted partitions
cat /mnt/etc/mkinitcpio.conf | grep -E '^HOOKS' > original_hooks.txt
if [ -n $ZFS ]; then
  sed -i \
    's/^HOOKS.*/HOOKS=(base udev keyboard keymap autodetect modconf block encrypt zfs filesystems fsck)/' \
    /mnt/etc/mkinitcpio.conf
else
  sed -i \
    's/^HOOKS.*/HOOKS=(base udev keyboard keymap autodetect modconf block encrypt lvm2 filesystems fsck)/' \
    /mnt/etc/mkinitcpio.conf
fi

# Install the bootloader
arch-chroot /mnt bootctl install

# Enable SSH and nm
arch-chroot /mnt systemctl enable sshd
arch-chroot /mnt systemctl enable NetworkManager.service

# Disable tmpfs for /tmp; we'll use zfs
if [ -n $ZFS ]; then arch-chroot /mnt systemctl mask tmp.mount; fi

# set up wifi
if [ "$wifi" == "true" ]; then
cat <<EOF > /mnt/etc/NetworkManager/system-connections/$wifi_ssid.nmconnection
[connection]
id=$wifi_ssid
type=wifi

[wifi]
mode=infrastructure
ssid=$wifi_ssid

[wifi-security]
auth-alg=open
key-mgmt=wpa-psk
psk=$wifi_password

[ipv4]
dns-search=
method=auto

[ipv6]
addr-gen-mode=stable-privacy
dns-search=
method=auto
EOF
fi

cat <<EOF > /mnt/boot/loader/loader.conf
default arch
timeout 3
EOF

boot_options="cryptdevice=PARTUUID=$(blkid -s PARTUUID -o value "$part_root"):cryptroot"

if [ -n $ZFS ]; then
  boot_options="$boot_options zfs=zroot rw"
else
  boot_options="$boot_options root=/dev/lvmroot/root rw"
fi

product_name=$(dmidecode \
  | grep -A 3 'System Information' \
  | grep 'Product Name' \
  | cut -d : -f 2 \
  | tr -d '[:blank:]')

if [ "${product_name}" == "XPS 15 9560" ]; then
  boot_options="${boot_options} nouveau.modeset=0 acpi_rev_override=1"
fi

# write the bootloader entry
cat <<EOF > /mnt/boot/loader/entries/arch.conf
title    Arch Linux
linux    /vmlinuz-linux
initrd   /initramfs-linux.img
options  $boot_options
EOF

# Set the timezone
arch-chroot /mnt ln -sf /usr/share/zoneinfo/America/Los_Angeles /etc/localtime

# Sync time
arch-chroot /mnt hwclock --systohc

# Set up tho locale
echo "en_US.UTF-8 UTF-8" > /mnt/etc/locale.gen
arch-chroot /mnt locale-gen
echo "LANG=en_US.UTF-8" > /mnt/etc/locale.conf
echo "KEYMAP=dvorak" > /mnt/etc/vconsole.conf

# Rebuild the initramfs image, after setting languages so we pick up the right
# keyboard layout for the console
arch-chroot /mnt mkinitcpio -p linux

arch-chroot /mnt useradd -mU -G wheel,uucp,video,audio,storage,games,input "$user"

if [ -n $ZFS ]; then
cat <<EOF >/mnt/home/$user/first-boot.sh
#!/bin/bash

set -euo pipefail

echo "


!! WE WILL NOW PERFORM FIRST-BOOT CONFIGURATION OF THE ZFS SYSTEM !!
   DO NOT SKIP THIS OR YOUR SYSTEM WILL FAIL TO BOOT AGAN


"

sudo zpool set cachefile=/etc/zfs/zpool.cache zroot
sudo systemctl enable zfs.target
sudo systemctl enable zfs-import-cache
sudo systemctl enable zfs-mount
sudo systemctl enable zfs-import.target
sudo zgenhostid \$(hostid)
sudo mkinitcpio -p linux

echo "


WE WILL NOW SWITCH FROM zfs-linux TO zfs-dkms TO UNBLOCK KERNEL UPGRADES.
PLEASE ALLOW zfs-linux TO BE REMOVED AND zfs-dkms INSTALLED. REBOOT AFTERWARDS.


"

sudo pacman -Syu zfs-dkms

echo "


First run configuration has been completed. It's suggested that you restart
before proceeding to do anything else. You can find the script that was used to
install the system in \$(pwd).

"

EOF
chmod +x /mnt/home/$user/first-boot.sh

cat <<EOF >>/mnt/home/$user/.bashrc
# Run the first-boot.sh script on first boot, delete it, and clear the line from
# bashrc
~/first-boot.sh && rm ~/first-boot.sh && sed -i '/first-boot/d' ~/.bashrc
EOF

# Make sure everything in the user's home directory is owned by them
arch-chroot /mnt chown -R $user: /home/$user
fi

# Set up the wheel group to have sudo access
echo "%wheel ALL=(ALL) ALL" >> /mnt/etc/sudoers

echo "$user:$password" | chpasswd --root /mnt
echo "root:$password" | chpasswd --root /mnt

# Copy this script into the new installation for reference
cp $0 /mnt/home/$user/$(basename $0)

if [ -n $ZFS ]; then
  umount /mnt/boot
  zfs umount -a
  zpool export zroot
fi

cat <<EOF



Installation complete!

You may now unmount everything and reboot, or enter the installed environment
via arch-chroot /mnt.
EOF

