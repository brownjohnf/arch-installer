use crate::{exec, Device};
use anyhow::{format_err, Result};
use cmd_lib::run_fun;
use std::fmt;

#[derive(Clone, Copy, Debug)]
pub(crate) struct ZFS {}

impl super::Filesystem for ZFS {
    fn cleanup(&self) -> Result<()> {
        crate::exec(&["umount", "/mnt/boot"])?;
        crate::exec(&["zfs", "umount", "-a"])?;
        crate::exec(&["zpool", "destroy", "zroot"])?;

        Ok(())
    }

    fn assert_dependencies(&self) -> Result<()> {
        // Ensure the ZFS module is present
        if run_fun!(modprobe zfs).is_err() {
            return Err(format_err!("zfs module is not loaded"));
        }

        Ok(())
    }

    fn init(&self, partition: &Device) -> Result<Self> {
        if !exec(&[
            "dd",
            "if=/dev/urandom",
            &format!("of={}", partition.dev()),
            "bs=512",
            "count=20480",
        ])?
        .status
        .success()
        {
            return Err(format_err!(
                "error using dd to overwrite beginning of {}",
                partition.dev()
            ));
        }

        if !exec(&[
            "zpool",
            "create",
            "-f",
            "zroot",
            "-m",
            "none",
            &partition.dev(),
        ])?
        .status
        .success()
        {
            return Err(format_err!(
                "error initializing zpool on {}",
                partition.dev()
            ));
        }

        Ok(Self {})
    }
}

impl fmt::Display for ZFS {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ZFS",)
    }
}
