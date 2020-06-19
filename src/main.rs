use anyhow::{anyhow, Context, Result};
use log::{debug, error, info};
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
    #[structopt(short, long, default_value = "zfs")]
    filesystem: Filesystem,

    // Whether or not to clean up from a previous failed run.
    #[structopt(long)]
    clean: bool,

    // Hostname for the system. Will generate a random one if not set.
    #[structopt(short, long)]
    hostname: Option<String>,

    // Whether or not to attempt to set up wifi.
    #[structopt(short, long)]
    wifi: bool,

    // Name of the default user. Will generate a random one if not set.
    #[structopt(short, long)]
    username: Option<String>,
}

#[derive(Debug)]
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
    type Err = FsError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "ext4" => Self::Ext4,
            "zfs" => Self::ZFS,
            _ => return Err(FsError("unknown fs".to_string())),
        })
    }
}

#[derive(Debug)]
struct FsError(String);

impl fmt::Display for FsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Error for FsError {}

#[derive(Debug)]
struct ExecError(String);

impl fmt::Display for ExecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Error for ExecError {}

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
    let filesystem = opt.filesystem;
    debug!("filesystem: {}", filesystem);
    let clean = opt.clean;
    debug!("clean up: {}", filesystem);
    let hostname = if let Some(f) = opt.hostname {
        f
    } else {
        // TODO: generate hostname
        "testarch".to_string()
    };
    debug!("hostname: {}", filesystem);
    let wifi = opt.wifi;
    debug!("configure wifi: {}", filesystem);
    let user = if let Some(u) = opt.username {
        u
    } else {
        "testuser".to_string()
    };
    debug!("default user: {}", filesystem);

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

    // Get a list of block devices so the user can select where they'd like to
    // install Arch.
    let devices = devices()?;

    Ok(())
}

fn devices() -> Result<Vec<(String, usize)>> {
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

        let name = path.file_name().unwrap().to_str().unwrap().to_string();

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
use cursive::align::HAlign;
use cursive::event::EventResult;
use cursive::traits::*;
use cursive::views::{Dialog, OnEventView, SelectView, TextView};
use cursive::Cursive;

// We'll use a SelectView here.
//
// A SelectView is a scrollable list of items, from which the user can select
// one.

fn select(items: &[String]) -> Result<String> {
    let mut out = String::new();

    let mut select = SelectView::new()
        // Center the text horizontally
        .h_align(HAlign::Center)
        // Use keyboard to jump to the pressed letters
        .autojump();

    // Populate the select list with the input.
    select.add_all_str(items);

    // Sets the callback for when "Enter" is pressed.
    select.set_on_submit(|s, device| {
        out = device;
        s.quit();
    });

    // Let's override the `j` and `k` keys for navigation
    let select = OnEventView::new(select)
        .on_pre_event_inner('k', |s, _| {
            s.select_up(1);
            Some(EventResult::Consumed(None))
        })
        .on_pre_event_inner('j', |s, _| {
            s.select_down(1);
            Some(EventResult::Consumed(None))
        });

    let mut siv = cursive::default();

    // Let's add a ResizedView to keep the list at a reasonable size
    // (it can scroll anyway).
    siv.add_layer(
        Dialog::around(select.scrollable().fixed_size((20, 10)))
            .title("Select a target install disk"),
    );

    siv.run();

    Ok(out);
}

// Let's put the callback in a separate function to keep it clean,
// but it's not required.
fn show_next_window(siv: &mut Cursive, city: &str) {
    siv.pop_layer();
    let text = format!("{} is a great city!", city);
    siv.add_layer(Dialog::around(TextView::new(text)).button("Quit", |s| s.quit()));
}
