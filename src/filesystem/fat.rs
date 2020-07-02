use crate::{exec, Device};
use anyhow::{format_err, Result};
use std::fmt;

#[derive(Clone, Copy, Debug)]
pub(crate) struct FAT32 {}

impl super::Filesystem for FAT32 {
    fn init(&self, partition: &Device) -> Result<Self> {
        if !exec(&["mkfs.vfat", "-F32", &format!("of={}", partition.dev())])?
            .status
            .success()
        {
            return Err(format_err!(
                "error formatting filesystem {} as FAT32",
                partition.dev()
            ));
        }

        Ok(Self {})
    }
}

impl fmt::Display for FAT32 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "FAT32",)
    }
}
