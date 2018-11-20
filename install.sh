#!/bin/bash

# WARNING: this script will destroy data on the selected disk.

set -euo pipefail
trap 's=$?; echo "$0: Error on line "$LINENO": $BASH_COMMAND"; exit $s' ERR

# If CLEAN is set, clean up any broken state we may have lying around
if [ -n $CLEAN ]; then
  set +e

  umount /mnt/boot
  umount /mnt/var
  umount /mnt/home
  umount /mnt/data
  umount /mnt

  vgchange -a n lvmroot
  pvremove -ff /dev/mapper/cryptroot

  set -e

  cryptsetup close /dev/mapper/cryptroot
fi

# Ensure we've got the system booted in EFI
ls /sys/firmware/efi/efivars > /dev/null

#REPO_URL="https://s3.eu-west-2.amazonaws.com/mdaffin-arch/repo/x86_64"
MIRRORLIST_URL="https://www.archlinux.org/mirrorlist/?country=US&protocol=https&use_mirror_status=on"

pacman -Sy --noconfirm pacman-contrib

# Only rank the mirrors if we're not cleaning up, indicating this is a fresh run
if [ -z $CLEAN ]; then
  echo "Updating mirror list"
  curl -s "$MIRRORLIST_URL" | \
      sed -e 's/^#Server/Server/' -e '/^#/d' | \
      rankmirrors -n 5 - > /etc/pacman.d/mirrorlist
fi

### Get infomation from user ###
hostname=$(dialog --stdout --inputbox "Enter hostname" 0 0) || exit 1
clear
: ${hostname:?"hostname cannot be empty"}

user=$(dialog --stdout --inputbox "Enter admin username" 0 0) || exit 1
clear
: ${user:?"user cannot be empty"}

password=$(dialog --stdout --inputbox "Enter admin password (will print)" 0 0) || exit 1
clear
: ${password:?"password cannot be empty"}

luks_password=$(dialog --stdout --inputbox "Enter LUKS password (will print)" 0 0) || exit 1
clear
: ${luks_password:?"luks_password cannot be empty"}

devicelist=$(lsblk -dplnx size -o name,size | grep -Ev "boot|rpmb|loop" | tac)
device=$(dialog --stdout --menu "Select installtion disk" 0 0 0 ${devicelist}) || exit 1
clear

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

# Set up the EFI partition
mkfs.vfat -F32 "${part_boot}"

cryptsetup -v luksFormat --type luks2 $part_root <<< "${luks_password}"
cryptsetup open $part_root cryptroot <<< "${luks_password}"

# Set up LVM
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

### Install and configure the basic system ###

#cat >>/etc/pacman.conf <<EOF
#[mdaffin]
#SigLevel = Optional TrustAll
#Server = $REPO_URL
#EOF

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

pacstrap /mnt base sudo zsh # base-devel networkmanager
genfstab -t PARTUUID /mnt >> /mnt/etc/fstab
cat /mnt/etc/fstab

hostname_short=$(echo $hostname | cut -d '.' -f 1)
cat <<EOF > /mnt/etc/hosts
127.0.0.1 localhost
::1       localhost
::1       $hostname $hostname_short
127.0.1.1	$hostname $hostname_short
EOF

#cat >>/mnt/etc/pacman.conf <<EOF
#[mdaffin]
#SigLevel = Optional TrustAll
#Server = $REPO_URL
#EOF

# Set up the hooks correctly for allowing us to unlock the encrypted partitions
cat /mnt/etc/mkinitcpio.conf | grep -E '^HOOKS' > original_hooks.txt
sed -i \
  's/^HOOKS.*/HOOKS=(base udev keyboard keymap autodetect modconf block encrypt lvm2 filesystems fsck)/' \
  /mnt/etc/mkinitcpio.conf

# Install the bootloader
arch-chroot /mnt bootctl install

# Rebuild the initramfs image
arch-chroot /mnt mkinitcpio -p linux

cat <<EOF > /mnt/boot/loader/loader.conf
default arch
EOF

cat <<EOF > /mnt/boot/loader/entries/arch.conf
title    Arch Linux
linux    /vmlinuz-linux
initrd   /initramfs-linux.img
options  cryptdevice=PARTUUID=$(blkid -s PARTUUID -o value "$part_root"):cryptroot root=/dev/lvmroot/root rw
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

arch-chroot /mnt useradd -mU -s /usr/bin/zsh -G wheel,uucp,video,audio,storage,games,input "$user"
touch /mnt/home/$user/.zshrc
arch-chroot /mnt chown $user: /home/$user/.zshrc

# Set up the wheel group to have sudo access
echo "%wheel ALL=(ALL) ALL" >> /mnt/etc/sudoers

# Change root user's shell to zsh
#arch-chroot /mnt chsh -s /usr/bin/zsh

echo "$user:$password" | chpasswd --root /mnt
echo "root:$password" | chpasswd --root /mnt

cat <<EOF



Installation complete!

You may now unmount everything and reboot, or enter the installed environment
via arch-chroot /mnt.
EOF

