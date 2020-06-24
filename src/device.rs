use anyhow::Result;
use log::{debug, error, info, warn};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub(crate) struct Device {
    pub(crate) name: String,
    pub(crate) bytes: usize,
}

impl Device {
    pub(crate) fn dev(&self) -> String {
        format!("/dev/{}", self.name)
    }

    pub(crate) fn from_path<T: AsRef<Path>>(path: T) -> Self {
        Self {
            name: path
                .as_ref()
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .to_string(),
            bytes: 0,
        }
    }

    pub(crate) fn partitions(&self) -> Result<Vec<Self>> {
        // Get the device id for this device.
        let id =
            fs::read_to_string(PathBuf::from("/sys/block/").join(&self.name).join("dev"))?.trim();

        // Read the symlink to the device location.
        let path = fs::read_link(
            PathBuf::from("/sys/class/block")
                .join(&self.name)
                .join("subsystem"),
        )?;

        // Grab all the partitions for the device.
        let mut partitions = vec![];
        for entry in path.read_dir()? {
            let path = entry?.path();
            let path = fs::read_link(path)?;

            let partition: usize = match fs::read_to_string(path) {
                Ok(p) => p.trim().parse()?,
                Err(e) => match e.kind() {
                    std::io::ErrorKind::NotFound => continue,
                    _ => return Err(anyhow::Error::new(e)),
                },
            };
        }

        Ok(partitions)
    }

    pub(crate) fn list() -> Result<Vec<Self>> {
        let mut out = vec![];

        let block = PathBuf::from("/sys/dev/block");
        for entry in fs::read_dir("/sys/block")? {
            let entry = entry?;
            let mut path = entry.path();

            // Skip this device if it's hidden.
            if fs::read_to_string(path.join("hidden"))?.trim() == "1" {
                continue;
            }

            // Get the size of the device.
            let size: usize = fs::read_to_string(path.join("size"))?.trim().parse()?;

            // Get the device ID for the device.
            let device = fs::read_to_string(path.join("dev"))?;
            let device = device.trim();
            let block = block.join(device);

            // Get the size of the device.
            let bytes: usize = fs::read_to_string(block.join("size"))?.trim().parse()?;
            if bytes < 1 {
                continue;
            }

            let name = String::from(path.file_name().unwrap().to_str().unwrap().to_string());

            out.push(Self { name, bytes });
        }

        out.sort_by(|a, b| b.bytes.cmp(&a.bytes));
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_list() {
        eprintln!("{:?}", Device::list().unwrap());
    }

    #[test]
    fn test_device_partitions() {
        let devices = Device::list().unwrap();
        devices[0].partitions().unwrap();
    }

    #[test]
    fn test_device_from_path() {
        assert_eq!(Device::from_path("/dev/sda").name, "sda");
    }
}
