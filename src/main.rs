use anyhow::{anyhow, Context, Result};
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

    // Execute as dry-run; parse the options but don't touch the system.
    #[structopt(short = "n", long)]
    dry_run: bool,
}

// Set the mirrorlist for install.
// TODO: Accept country and other options?
const MIRRORLIST_URL: &str =
    "https://www.archlinux.org/mirrorlist/?country=US&protocol=https&use_mirror_status=on";

fn main() {
    CombinedLogger::init(vec![
        WriteLogger::new(
            LevelFilter::Debug,
            Config::default(),
            fs::File::create("/tmp/arch-installer.log").unwrap(),
        ),
        TermLogger::new(LevelFilter::Debug, Config::default(), TerminalMode::Mixed),
    ])
    .unwrap();

    let opt = Opt::from_args();
    debug!("{:?}", opt);

    if let Err(e) = run(opt) {
        error!("{}", e);
        process::exit(1);
    }
}

fn run(opt: Opt) -> Result<()> {
    // Set up all our inputs
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
    let user = if let Some(u) = opt.username {
        u
    } else {
        input("Default username?")?
    };
    debug!("default user: {}", user);
    // Select the device
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
        dev.remove(i)
    };
    debug!("device: {:?}", device);

    if opt.dry_run {
        warn!("running in dry-run mode; exiting now");
        return Ok(());
    }

    if confirm("Installation will be destructive; continue?")? {
        error!("no confirmation received; aborting");
        process::exit(1);
    }

    // Clean the system if was requested.
    if clean {
        filesystem.cleanup()?;
    }

    // Ensure the system is booted in EFI.
    assert_efi()?;

    // Ensure the ZFS module is present
    if !exec(&["modprobe", "zfs"])?.status.success() {
        return Err(anyhow!("zfs module is not loaded"));
    }

    // Rank the mirrors, but only if we're not cleaning up, indicating this is a
    // fresh run. This is because ranking takes a while.
    // TODO: Do all this straight in rust using reqwest or something.
    if !clean {
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
            return Err(anyhow!("error setting up mirrors: {:?}"));
        }
    }

    // Set the datetime from NTP.
    if !exec(&["timedatectl", "set-ntp", "true"])?.status.success() {
        return Err(anyhow!("error setting system time from ntp"));
    }

    // Partition the disk. Make a large /boot partition so that we have room for
    // multiple kernels, etc.
    if !exec(&[
        "parted",
        "--script",
        &device.dev(),
        "--",
        "
  mklabel gpt \
  mkpart ESP fat32 1Mib 2GiB \
  set 1 boot on \
  mkpart primary ext4 2GiB 100%",
    ])?
    .status
    .success()
    {
        return Err(anyhow!("error partitioning disk"));
    }

    // Get our partitions.
    let parts = device.partitions()?;
    let part_boot = &parts[0];
    let part_root = &parts[1];

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

    Err(anyhow!("system does not appear to be booted in EFI mode"))
}

fn exec(cmd: &[&str]) -> Result<process::Output> {
    debug!("exec: running: {:?}", cmd);

    let (cmd, args) = match cmd {
        [cmd, args @ ..] => (cmd, args),
        _ => return Err(anyhow!("missing command".to_string())),
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
