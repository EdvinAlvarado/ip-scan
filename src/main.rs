use clap::Parser;
use ipnet::Ipv4AddrRange;
use main_error::MainError;
use std::path::PathBuf;
use std::process::Command;
use std::thread;
use thiserror::Error;

#[derive(Error, Debug)]
enum ScanError {
    #[error("Wrong Arguments. Check whether input method was mixed.")]
    WrongArguments,
    #[error("Ping command incorrect or no ping exe available.")]
    PingCommandError(#[from] std::io::Error),
}

fn ping<S: AsRef<std::ffi::OsStr>>(ip: S, count: Option<u32>) -> Result<bool, ScanError> {
    let mut cmd = Command::new("ping");
    cmd.arg(ip);
    if let Some(c) = count {
        cmd.args(["-n", &c.to_string()]);
    }

    let exit_code = cmd.output().map_err(ScanError::PingCommandError)?.status;
    Ok(exit_code.success())
}

macro_rules! ping {
    ($a: expr) => {
        ping($a, None)
    };
    ($a: expr, $b: expr) => {
        ping($a, $b)
    };
}

/// Scan ips or hostnames to see if pingable.
/// Scan an IP range from to.
/// For hostnames, pipe a list of hostnames/ips or pass a file including lines of hostnames/ips.
#[derive(Parser)]
#[command(author, version, about)]
struct Args {
    /// start IP address
    from: Option<String>,
    /// end IP address
    to: Option<String>,
    /// Read hostnames/ips from this file
    #[arg(short, long)]
    file: Option<PathBuf>,
    /// Flag to read from pipe.
    #[arg(short, long, action=clap::ArgAction::SetTrue)]
    pipe: bool,
}

fn main() -> Result<(), MainError> {
    let args = Args::parse();
    let hosts: Vec<String> = match (args.from, args.to, args.file, args.pipe) {
        (Some(from), Some(to), None, false) => 
			Ipv4AddrRange::new(from.parse()?, to.parse()?)
			.map(|ip| ip.to_string())
			.collect(),
        (None, None, Some(file_path), false) => 
			std::fs::read_to_string(file_path)?
            .lines()
            .map(|s| s.to_string())
            .collect(),
        (None, None, None, true) =>
			std::io::stdin().lines()
			.map(|ol| ol.unwrap())
			.collect(),
        _ => {
            return Err(ScanError::WrongArguments.into());
        }
    };

    let mut threads = vec![];
    for host in hosts {
        let t = thread::spawn(move || {
            if ping!(&host).unwrap() {
                println!("{}", host);
                return Some(host);
            }
            None
        });
        threads.push(t);
    }
    let _answered_hosts: Vec<String> = threads
        .into_iter()
        .map(|t| t.join().unwrap())
        .filter_map(|o| o)
        .collect();

    Ok(())
}
