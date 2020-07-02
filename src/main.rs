use anyhow::{anyhow, format_err, Context, Result};
use dialoguer::{theme::ColorfulTheme, Confirm, Input, Select};
use log::{debug, error, info, warn};
use simplelog::{CombinedLogger, Config, LevelFilter, TermLogger, TerminalMode, WriteLogger};
use std::{
    error::Error,
    fmt, fs,
    path::{Path, PathBuf},
    process,
    str::FromStr,
};
use structopt::StructOpt;

mod device;
mod filesystem;
#[cfg(test)]
mod tests;

use device::Device;
use filesystem::Filesystem;

#[derive(Debug, StructOpt)]
#[structopt(setting = structopt::clap::AppSettings::ColoredHelp)]
#[structopt(rename_all = "kebab-case")]
struct Opt {
    /// Which filesystem to use for the installation.
    #[structopt(short, long)]
    filesystem: Option<String>,

    // Whether or not to clean up from a previous failed run.
    #[structopt(long)]
    clean: bool,

    // Hostname for the system. Will ask for one if not set.
    #[structopt(short, long)]
    hostname: Option<String>,

    // Whether or not to attempt to set up wifi.
    #[structopt(short, long)]
    wifi: Option<bool>,

    // Name of the default user. Will generate a random one if not set.
    #[structopt(short, long)]
    username: Option<String>,

    // Path to the device to install the system on. Will prompt if not passed.
    #[structopt(short, long)]
    device: Option<PathBuf>,
}

// Set the mirrorlist for install.
// TODO: Accept country and other options?
const MIRRORLIST_URL: &str =
    "https://www.archlinux.org/mirrorlist/?country=US&protocol=https&use_mirror_status=on";

fn main() -> Result<()> {
    // Set up the logger to log to terminal and disk, for debugging later.
    CombinedLogger::init(vec![
        WriteLogger::new(
            LevelFilter::Debug,
            Config::default(),
            fs::File::create("arch-installer.log")?,
        ),
        TermLogger::new(LevelFilter::Debug, Config::default(), TerminalMode::Mixed),
    ])
    .unwrap();

    // Parse the arguments.
    let opt = Opt::from_args();
    debug!("{:?}", opt);

    // Configure the arguments, setting defaults and such.
    let filesystem = if let Some(f) = opt.filesystem {
        f
    } else {
        let options = &["zfs".to_string()];
        let i = select(
            "What filesystem would you like to use?",
            &["zfs".to_string()],
        )?;

        options[i].to_string()
    };
    let filesystem = filesystem::from_str(filesystem)?;
    debug!("filesystem: {}", filesystem);

    let clean = opt.clean;
    debug!("clean up: {}", clean);

    let hostname = if let Some(f) = opt.hostname {
        f
    } else {
        input("What should the hostname be?")?
    };
    debug!("hostname: {}", hostname);

    let wifi = if let Some(w) = opt.wifi {
        w
    } else {
        confirm("Would you like to configured WiFi?")?
    };
    debug!("configure wifi: {}", wifi);

    // Set the user to set up as the default non-privileged user.
    let user = if let Some(u) = opt.username {
        u
    } else {
        input("Default username?")?
    };
    debug!("default user: {}", user);

    // Select the device we're going to install the OS onto.
    let device = if let Some(d) = opt.device {
        Device::from_path(d)
    } else {
        let mut dev = Device::list()?;
        let i = select(
            "Select installation device",
            &dev.iter()
                .map(|d| format!("/dev/{} {}", d.name, d.bytes))
                .collect::<Vec<String>>(),
        )?;

        // Warning: this will panic if i is out of bounds.
        // TODO: Come up with a better way to get this data.
        dev.remove(i)
    };
    debug!("device: {:?}", device);

    // Ensure the system is booted in EFI.
    assert_efi()?;

    // Ensure the ZFS module is present
    if !exec(&["modprobe", "zfs"])?.status.success() {
        return Err(anyhow!("zfs module is not loaded"));
    }

    // Make sure the user understands this is going to be destructive.
    if !confirm("Installation will be destructive; continue?")? {
        error!("no confirmation received; aborting");
        process::exit(1);
    }

    // Set the datetime from NTP.
    if !exec(&["timedatectl", "set-ntp", "true"])?.status.success() {
        return Err(anyhow!("error setting system time from ntp"));
    }

    // Rank the mirrors, but only if we're not cleaning up, indicating this is a
    // fresh run. This is because ranking takes a while. This will just write to
    // the ISO.
    if !clean {
        rankmirrors()?;
    }

    // Clean up the system if was requested. This is mostly needed if the
    // installer has been run before unsuccessfully.
    if clean {
        filesystem.cleanup()?;
    }

    // Partition the disk. Make a large /boot partition so that we have room for
    // multiple kernels, etc.
    partition(&device)?;

    // Get our partitions.
    let parts = device.partitions()?;
    let part_boot = &parts[0];
    let part_root = &parts[2];

    // Get rid of any old partition/filesystem info from the partitions.
    wipe(part_boot)?;
    wipe(part_root)?;

    // Set up the boot partition filesystem.
    let fat32 = filesystem::FAT32 {};
    fat32.init(part_boot)?;

    // Set up the root partition filesystem.
    filesystem.init(part_root)?;

    Ok(())
}

// Wipe any existing data from the partitions. This will get rid of any extant
// file contents which which still be around even after wiping the inode data.
fn wipe(partition: &Device) -> Result<()> {
    if !exec(&["wipefs", &partition.dev()])?.status.success() {
        return Err(format_err!("error wiping partition {:?}", partition));
    }

    Ok(())
}

/// Create partitions.
fn partition(device: &Device) -> Result<()> {
    if !exec(&[
        "parted",
        "--script",
        &device.dev(),
        "--",
        // Make the partition table.
        "mklabel",
        "gpt",
        // Make the boot partition for EFI.
        "mkpart",
        "ESP",
        "fat32",
        "1Mib",
        "2GiB",
        "set",
        "1",
        "boot",
        "on",
        // Make a persistent small partition for things like encrypted storage.
        "mkpart",
        "primary",
        "ext4",
        "2GiB",
        "3GiB",
        // Make the ZFS root partition.
        "mkpart",
        "primary",
        "ext4",
        "3GiB",
        "100%",
    ])?
    .status
    .success()
    {
        return Err(format_err!("error partitioning disk"));
    }

    Ok(())
}

fn confirm(title: &str) -> Result<bool> {
    Ok(Confirm::new().with_prompt(title).interact()?)
}

fn input(title: &str) -> Result<String> {
    Ok(Input::<String>::new().with_prompt(title).interact()?)
}

fn assert_efi() -> Result<()> {
    if fs::metadata("/sys/firmware/efi/efivars")?.is_dir() {
        return Ok(());
    }

    Err(format_err!(
        "system does not appear to be booted in EFI mode"
    ))
}

fn exec(cmd: &[&str]) -> Result<process::Output> {
    debug!("exec: running: {:?}", cmd);

    let (cmd, args) = match cmd {
        [cmd, args @ ..] => (cmd, args),
        _ => return Err(format_err!("missing command".to_string())),
    };

    Ok(process::Command::new(cmd)
        .args(args)
        .output()
        .with_context(|| format!("{:?} {:?}", cmd, args))?)
}

/// Allow the user to interactively select an item.
fn select(title: &str, items: &[String]) -> Result<usize> {
    Ok(Select::with_theme(&ColorfulTheme::default())
        .with_prompt(title)
        .default(0)
        .items(items)
        .interact()?)
}

/// Calculate the most efficient mirrors to use with pacman. This can make a
/// huge difference on install time, as well as normal system updates.
// TODO: Do all this straight in rust using reqwest or something.
fn rankmirrors() -> Result<()> {
    if !exec(&[
            "bash",
            "-c",
            &format!(
                "curl -s {} | sed -e 's/^#Server/Server/' -e '/^#/d' | rankmirrors -n 5 - > /etc/pacman.d/mirrorlist",
                MIRRORLIST_URL
            ),
        ])?
        .status
        .success()
        {
            return Err(format_err!("error setting up mirrors: {:?}"));
        }

    Ok(())
}
