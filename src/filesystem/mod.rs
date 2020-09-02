use crate::Device;
use anyhow::{format_err, Result};
use std::fmt::{Debug, Display};

mod ext4;
mod fat;
mod zfs;

pub(crate) use ext4::Ext4;
pub(crate) use fat::FAT32;
pub(crate) use zfs::ZFS;

pub(crate) trait Filesystem: Clone + Copy + Debug + Display {
    fn cleanup(&self) -> Result<()> {
        Ok(())
    }

    fn assert_dependencies(&self) -> Result<()> {
        Ok(())
    }

    fn init(&self, partition: &Device) -> Result<Self>;
}

pub(crate) fn from_str<T: AsRef<str>>(s: T) -> Result<impl Filesystem> {
    Ok(match s.as_ref() {
        "zfs" => ZFS {},
        _ => return Err(format_err!("unknown fs".to_string())),
    })
}
