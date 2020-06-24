use anyhow::{format_err, Result};
use std::fmt::{Debug, Display};

mod zfs;

pub(crate) use zfs::ZFS;

pub(crate) trait Filesystem: Clone + Copy + Debug + Display {
    fn cleanup(&self) -> Result<()>;
}

pub(crate) fn from_str<T: AsRef<str>>(s: T) -> Result<impl Filesystem> {
    Ok(match s.as_ref() {
        "zfs" => ZFS {},
        _ => return Err(format_err!("unknown fs".to_string())),
    })
}
