#!/bin/sh

set -eux

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
  mfkfs.ext4 $1

  mount $1 /mnt
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
function additional_boot_options() {
  true
}

function first_boot_script() {
  true
}
