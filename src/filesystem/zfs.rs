use anyhow::Result;
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
}

impl fmt::Display for ZFS {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ZFS",)
    }
}
