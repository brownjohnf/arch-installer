#!/bin/sh

set -eu

# This function unmounts all filesystems, preparatory for rebooting.
function umount_all() {
  umount /mnt/boot
  umount /mnt
}

function clean() {
  umount_all || true
}

# No deps needed for ext4
function checkdeps() {
  true
}

# This should format and mount the root partition(s)
function format_root() {
  # Create a pool made up of the drive
  mkfs.ext4 $1

  mount -t ext4 $1 /mnt
}

function make_fstab() {
  # Generate an fstab and put it in the chroot environment. We only want the boot
  # partition in it; the rest gets handled by ZFS
  genfstab -t PARTUUID /mnt > /mnt/etc/fstab
}

function hooks() {
  echo ""
}

# Add packages before installing the bootloader and restarting.
function add_packages() {
  true
}

# Set default boot options.
# TODO: Accept part UUID
#
# $1: Root partition
function additional_boot_options() {
  echo root=$1 rw
}

function first_boot_script() {
  true
}
