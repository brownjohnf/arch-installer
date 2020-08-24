#!/bin/sh

set -eux

# This function unmounts all filesystems, preparatory for rebooting.
function umount_all() {
  umount /mnt/boot
  zfs umount -a
  zpool export zroot
}

function clean() {
  umount_all || true
  zpool destroy zroot || true
}

function checkdeps() {
  # Ensure we have the zfs module
  modprobe zfs
}

# This should format and mount the root partition(s)
function format_root() {
  # Create a pool made up of the drive
  zpool create -f zroot -m none $1

  # Create the root (/), with default attributes (|| true because it will fail to
  # mount)
  # TODO: Figure out a better way to recover from the mount failure. Catching any
  # error is not a good idea.
  zfs create \
    -o atime=off \
    -o compression=on \
    -o mountpoint=/ \
    -o encryption=on \
    -o keyformat=passphrase \
    zroot/ROOT || true

  # Create datasets for all the other partitions we want to isolate, setting
  # their mountpoint (|| true because it will fail to mount)
  # TODO: Figure out a better way to recover from the mount failure. Catching any
  # error is not a good idea.
  for path in /home /var /var/log /var/log/journal /etc /data /data /docker; do
    zfs create -o mountpoint=$path zroot/ROOT$path || true
  done

  # Unmount everything for now
  zfs unmount -a

  # Set up posix ACL for the journal
  zfs set acltype=posixacl zroot/ROOT/var/log/journal
  zfs set xattr=sa zroot/ROOT/var/log/journal

  zpool set bootfs=zroot/ROOT zroot
  zpool export zroot

  # Import the pool by id, to ensure we get consistent mounting
  zpool import -l -d /dev/disk/by-id -R /mnt zroot
  # Set the cachefile for the pool, and then copy it over to the chroot fs
  zpool set cachefile=/etc/zfs/zpool.cache zroot
  mkdir -p /mnt/etc/zfs
  cp /etc/zfs/zpool.cache /mnt/etc/zfs/zpool.cache
}

function make_fstab() {
  # Generate an fstab and put it in the chroot environment. We only want the boot
  # partition in it; the rest gets handled by ZFS
  genfstab -t PARTUUID /mnt | grep -A 1 $1 > /mnt/etc/fstab
}

# Set up the hooks correctly for allowing us to unlock the encrypted partitions.
# Doing this here _should_ mean that when we install the zfs- modules below, the
# kernels' ramdisks etc. should pick it up.
function hooks() {
  echo base udev keyboard keymap autodetect modconf block encrypt zfs filesystems
}

# Add packages before installing the bootloader and restarting.
function add_packages() {
  # Install zfs modules
  cat <<EOF >>/mnt/etc/pacman.conf
[archzfs]
Server = https://archzfs.com/\$repo/x86_64
EOF

  arch-chroot /mnt pacman-key --recv-keys F75D9D76 --keyserver keyserver.ubuntu.com
  arch-chroot /mnt pacman-key --lsign-key F75D9D76
  arch-chroot /mnt pacman -Sy --needed --noconfirm zfs-linux zfs-linux-lts
}

# Set default boot options, which are pretty straightforward. This will attempt
# to mount the root fs from the zroot pool, and will prompt for encryption
# passwords as though you'd invoked 'zpool import -l zroot'.
function additional_boot_options() {
  echo zfs=zroot rw
}

function first_boot_script() {
  cat <<EOF
#!/bin/bash

set -euo pipefail

echo "


!! WE WILL NOW PERFORM FIRST-BOOT CONFIGURATION OF THE ZFS SYSTEM !!
   DO NOT SKIP THIS OR YOUR SYSTEM WILL FAIL TO BOOT AGAIN

   If you don't have an internet connection, sort that out and then
   run './first-boot.sh'. Afterwards, remove the final line from
   .bashrc so this doesn't run again.


"

# Check for internet
ping -c 1 github.com > /dev/null

sudo zpool set cachefile=/etc/zfs/zpool.cache zroot
sudo systemctl enable zfs.target
sudo systemctl enable zfs-import-cache
sudo systemctl enable zfs-mount
sudo systemctl enable zfs-import.target
sudo zgenhostid \$(hostid)
sudo mkinitcpio -p linux
sudo mkinitcpio -p linux-lts

echo "


First run configuration has been completed. It's suggested that you restart
before proceeding to do anything else. You can find the script that was used to
install the system in \$(pwd).

"

EOF
}
