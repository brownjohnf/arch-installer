use crate::{exec, Device};
use anyhow::{format_err, Result};
use cmd_lib::{run_cmd, run_fun};
use std::fmt;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Ext4 {}

impl super::Filesystem for Ext4 {
    fn cleanup(&self) -> Result<()> {
        run_fun!(umount / mnt / boot)?;
        run_fun!(umount / mnt)?;

        Ok(())
    }

    fn init(&self, partition: &Device) -> Result<Self> {
        run_fun!(mkfs.ext4 $partition)?;

        Ok(Self {})
    }
}

impl fmt::Display for Ext4 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ext4",)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::filesystem::Filesystem;

    #[test]
    fn test_init() {
        run_cmd!(truncate -s 900000 /tmp/foobar).unwrap();
        let fs = Ext4 {};
        fs.init(&Device::from_path("/tmp/foobar")).unwrap();
    }
}
