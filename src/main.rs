use anyhow::Result;
use log::debug;
use std::{error::Error, fmt, process, str::FromStr};
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

fn exec(cmd: &[&str]) -> Result<process::Output> {
    let (cmd, args) = match cmd {
        [cmd, args @ ..] => (cmd, args),
        _ => panic!("invalid cmd/arg input"),
    };

    Ok(process::Command::new(cmd).args(args).output()?)
}

fn main() {
    env_logger::init();

    let opt = Opt::from_args();
    debug!("{:?}", opt);

    // Set up all our inputs
    let filesystem = opt.filesystem;
    let clean = opt.clean;
    let hostname = if let Some(f) = opt.hostname {
        f
    } else {
        // TODO: generate hostname
        "testarch".to_string()
    };
    let wifi = opt.wifi;
    let user = if let Some(u) = opt.username {
        u
    } else {
        "testuser".to_string()
    };

    // Clean the system if was requested.
    cleanup(filesystem).unwrap();
}

fn cleanup(f: Filesystem) -> Result<()> {
    Ok(())
}
