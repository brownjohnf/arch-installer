use anyhow::{anyhow, Context, Result};
use dialoguer::{theme::ColorfulTheme, Confirm, Input, Select};
use log::{debug, error, info, warn};
use simplelog::{CombinedLogger, Config, LevelFilter, TermLogger, TerminalMode, WriteLogger};
use std::{error::Error, fmt, fs, path::PathBuf, process, str::FromStr};
use structopt::StructOpt;

#[cfg(test)]
mod tests;

#[derive(Debug, StructOpt)]
#[structopt(setting = structopt::clap::AppSettings::ColoredHelp)]
#[structopt(rename_all = "kebab-case")]
struct Opt {
    /// Which filesystem to use for the installation.
    #[structopt(short, long)]
    filesystem: Option<Filesystem>,

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

#[derive(Clone, Copy, Debug)]
enum Filesystem {
    Ext4,
    ZFS,
}

impl fmt::Display for Filesystem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Ext4 => "ext4",
                Self::ZFS => "zfs",
            }
        )
    }
}

impl FromStr for Filesystem {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "ext4" => Self::Ext4,
            "zfs" => Self::ZFS,
            _ => return Err(anyhow!("unknown fs".to_string())),
        })
    }
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
        let options = &[Filesystem::ZFS];
        let i = select(
            "What filesystem would you like to use?",
            &["zfs".to_string()],
        )?;

        options[i]
    };
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
        d
    } else {
        let dev = devices()?;
        let i = select(
            "Select installation device",
            &dev.iter()
                .map(|(path, bytes)| format!("{} {}", path.to_str().unwrap(), bytes))
                .collect::<Vec<String>>(),
        )?;

        PathBuf::from(&dev[i].0)
    };
    debug!("device: {:?}", device);

    if opt.dry_run {
        warn!("running in dry-run mode; exiting now");
        return Ok(());
    }

    panic!("safety third!");

    // Clean the system if was requested.
    if clean {
        cleanup(filesystem)?;
    }

    // Ensure the system is booted in EFI.
    assert_efi()?;

    // Ensure the ZFS module is present
    if !exec(&["modprobe", "zfs"])?.status.success() {
        return Err(anyhow!("zfs module is not loaded"));
    }

    // Rank the mirrors, but only if we're not cleaning up, indicating this is a
    // fresh run. This is because ranking takes a while.
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

    // Partition the disk.
    if !exec(&[
        "parted",
        "--script",
        device.to_str().unwrap(),
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

    Ok(())
}

fn confirm(title: &str) -> Result<bool> {
    Ok(Confirm::new().with_text(title).interact()?)
}

fn input(title: &str) -> Result<String> {
    Ok(Input::<String>::new().with_prompt(title).interact()?)
}

fn partitions_for_device(device: &Path) -> Result<Vec<PathBuf>> {
// Get the device id for this device.
let device = device.file_name()?;
let id = fs::read_to_string(PathBuf::from("/sys/block/").join(device).join("dev"))?.strip();
let path = PathBuf::from("/sys/dev/block")
        let mut partition = 1;
    while path.join(format!("{}:{}", disk, partition)).exists() {
        partition += 1;
    }
}

fn devices() -> Result<Vec<(PathBuf, usize)>> {
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
        let size: usize = fs::read_to_string(block.join("size"))?.trim().parse()?;
        if size < 1 {
            continue;
        }

        let name =
            PathBuf::from("/dev").join(path.file_name().unwrap().to_str().unwrap().to_string());

        out.push((name, size));
    }

    out.sort_by(|a, b| b.cmp(a));
    Ok(out)
}

fn assert_efi() -> Result<()> {
    if fs::metadata("/sys/firmware/efi/efivars")?.is_dir() {
        return Ok(());
    }

    Err(anyhow!("system does not appear to be booted in EFI mode"))
}

fn cleanup(f: Filesystem) -> Result<()> {
    debug!("cleaning up from any prior run that may have occurred");

    exec(&["umount", "/mnt/boot"])?;
    exec(&["zfs", "umount", "-a"])?;
    exec(&["zpool", "destroy", "zroot"])?;

    Ok(())
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
