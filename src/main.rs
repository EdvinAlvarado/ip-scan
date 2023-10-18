use clap::Parser;
use ipnet::Ipv4AddrRange;
use main_error::MainError;
use std::process::Command;
use std::thread;
use thiserror::Error;

#[derive(Error, Debug)]
enum ScanError {
    #[error("Missing Arguments")]
    MissingParameter,
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


/// scan ips or hostnames to see if pingable
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// start ip
    #[arg(short, long)]
    from: Option<String>,
    /// end ip
    #[arg(short, long)]
    to: Option<String>,
}


fn main() -> Result<(), MainError> {
    let args = Args::parse();
    let hosts: Vec<String> = match (args.from.is_some(), args.to.is_some()) {
        (true, true) => {
            let from = args.from.unwrap().parse()?;
            let to = args.to.unwrap().parse()?;
            Ipv4AddrRange::new(from, to)
                .map(|ip| ip.to_string())
                .collect()
        }
        (false, false) => {
            let stdin = std::io::stdin();
            stdin.lines().map(|ol| ol.unwrap()).collect()
        }
        (true, false) | (false, true) => {
            return Err(ScanError::MissingParameter.into());
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
