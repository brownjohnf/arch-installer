#!/bin/bash

# WARNING: this script will destroy data on the selected disk.

set -euo pipefail
trap 's=$?; echo "$0: Error on line "$LINENO": $BASH_COMMAND"; exit $s' ERR

# Ensure we've got the system booted in EFI
ls /sys/firmware/efi/efivars > /dev/null

#REPO_URL="https://s3.eu-west-2.amazonaws.com/mdaffin-arch/repo/x86_64"
MIRRORLIST_URL="https://www.archlinux.org/mirrorlist/?country=US&protocol=https&use_mirror_status=on"

pacman -Sy --noconfirm pacman-contrib

echo "Updating mirror list"
curl -s "$MIRRORLIST_URL" | \
    sed -e 's/^#Server/Server/' -e '/^#/d' | \
    rankmirrors -n 5 - > /etc/pacman.d/mirrorlist

### Get infomation from user ###
hostname=$(dialog --stdout --inputbox "Enter hostname" 0 0) || exit 1
clear
: ${hostname:?"hostname cannot be empty"}

user=$(dialog --stdout --inputbox "Enter admin username" 0 0) || exit 1
clear
: ${user:?"user cannot be empty"}

password=$(dialog --stdout --passwordbox "Enter admin password (will not print)" 0 0) || exit 1
clear
: ${password:?"password cannot be empty"}
password2=$( \
  dialog --stdout --passwordbox \
  "Enter admin password again (will not print)" 0 0 \
) || exit 1
clear
[[ "$password" == "$password2" ]] || ( echo "Passwords did not match"; exit 1; )

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
  mkpart primary ext4 129MiB 100%

# Simple globbing was not enough as on one device I needed to match /dev/mmcblk0p1
# but not /dev/mmcblk0boot1 while being able to match /dev/sda1 on other devices.
part_boot="$(ls ${device}* | grep -E "^${device}p?1$")"
part_root="$(ls ${device}* | grep -E "^${device}p?2$")"

wipefs "${part_boot}"
wipefs "${part_root}"

mkfs.vfat -F32 "${part_boot}"
mkfs.f2fs -f "${part_root}"

mount "${part_root}" /mnt
mkdir /mnt/boot
mount "${part_boot}" /mnt/boot

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
echo "${hostname}" > /mnt/etc/hostname

#cat >>/mnt/etc/pacman.conf <<EOF
#[mdaffin]
#SigLevel = Optional TrustAll
#Server = $REPO_URL
#EOF

arch-chroot /mnt bootctl install

cat <<EOF > /mnt/boot/loader/loader.conf
default arch
EOF

cat <<EOF > /mnt/boot/loader/entries/arch.conf
title    Arch Linux
linux    /vmlinuz-linux
initrd   /initramfs-linux.img
options  root=PARTUUID=$(blkid -s PARTUUID -o value "$part_root") rw
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

hostname_short=$(echo $hostname | cut -d '.' -f 1)
cat <<EOF > /mnt/etc/hosts
127.0.0.1 localhost
::1       localhost
::1       $hostname $hostname_short
127.0.1.1	$hostname $hostname_short
EOF

arch-chroot /mnt useradd -mU -s /usr/bin/zsh -G wheel,uucp,video,audio,storage,games,input "$user"
touch /mnt/home/$user/.zshrc
arch-chroot /mnt chown $user: /mnt/home/$user/.zshrc

# Change root user's shell to zsh
#arch-chroot /mnt chsh -s /usr/bin/zsh

echo "$user:$password" | chpasswd --root /mnt
echo "root:$password" | chpasswd --root /mnt

